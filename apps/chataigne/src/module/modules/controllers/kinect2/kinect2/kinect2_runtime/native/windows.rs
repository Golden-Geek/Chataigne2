use std::ffi::{c_void, OsStr};
use std::os::windows::ffi::OsStrExt;
use std::ptr;
use std::sync::OnceLock;

use windows_sys::core::GUID;
use windows_sys::Win32::Foundation::{FARPROC, HANDLE, WAIT_OBJECT_0, WAIT_TIMEOUT};
use windows_sys::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
use windows_sys::Win32::System::Threading::WaitForSingleObject;

use super::{
    KinectBodySnapshot, KinectFrameSnapshot, KinectJoint, KinectJointSample, KinectRuntimePoll, KinectTrackingState,
    KinectVec3,
};

const BODY_COUNT: usize = 6;

type Boolean = u8;
type HResult = i32;
type UInt = u32;
type ULong = u32;
type UInt64 = u64;
type Timespan = i64;
type WaitableHandle = HANDLE;

type GetDefaultKinectSensorFn = unsafe extern "system" fn(default_kinect_sensor: *mut *mut IKinectSensor) -> HResult;

static KINECT_API: OnceLock<Result<KinectApi, String>> = OnceLock::new();

pub(super) struct PlatformKinect2Runtime {
    sensor: KinectSensorHandle,
    reader: BodyFrameReaderHandle,
    frame_arrived_handle: WaitableHandle,
}

impl PlatformKinect2Runtime {
    pub(super) fn create() -> Result<Self, String> {
        let api = kinect_api()?;
        let sensor = KinectSensorHandle::open_default(api)?;
        let source = sensor.body_frame_source()?;
        let reader = source.open_reader()?;
        let mut frame_arrived_handle = ptr::null_mut();
        reader.subscribe_frame_arrived(&mut frame_arrived_handle)?;

        Ok(Self {
            sensor,
            reader,
            frame_arrived_handle,
        })
    }

    pub(super) fn poll(&mut self) -> Result<KinectRuntimePoll, String> {
        let sensor_available = self.sensor.is_available()?;
        if self.frame_arrived_handle.is_null() {
            return Err("Kinect 2 body-frame subscription handle is invalid.".to_string());
        }

        let wait_status = unsafe { WaitForSingleObject(self.frame_arrived_handle, 0) };
        if wait_status == WAIT_TIMEOUT {
            return Ok(KinectRuntimePoll {
                sensor_available,
                frame: None,
            });
        }
        if wait_status != WAIT_OBJECT_0 {
            return Err(format!(
                "Kinect 2 body-frame wait failed: {}",
                std::io::Error::last_os_error()
            ));
        }

        let event_args = self.reader.frame_arrived_event_data(self.frame_arrived_handle)?;
        let frame_reference = event_args.frame_reference()?;
        let frame = frame_reference.acquire_frame()?;

        Ok(KinectRuntimePoll {
            sensor_available,
            frame: Some(KinectFrameSnapshot {
                sensor_available,
                timestamp_100ns: frame_timestamp(frame.relative_time()?)?,
                tracked_bodies: frame.tracked_bodies()?,
            }),
        })
    }
}

impl Drop for PlatformKinect2Runtime {
    fn drop(&mut self) {
        if !self.frame_arrived_handle.is_null() {
            let _ = self.reader.unsubscribe_frame_arrived(self.frame_arrived_handle);
            self.frame_arrived_handle = ptr::null_mut();
        }
    }
}

struct KinectApi {
    get_default_sensor: GetDefaultKinectSensorFn,
}

fn kinect_api() -> Result<&'static KinectApi, String> {
    match KINECT_API.get_or_init(KinectApi::load) {
        Ok(api) => Ok(api),
        Err(error) => Err(error.clone()),
    }
}

impl KinectApi {
    fn load() -> Result<Self, String> {
        let library_name = wide_null("Kinect20.dll");
        let module = unsafe { LoadLibraryW(library_name.as_ptr()) };
        if module.is_null() {
            return Err(
                "Kinect20.dll was not found. Bundle the Kinect runtime DLL beside the executable or install the Kinect runtime on this Windows machine."
                    .to_string(),
            );
        }

        let proc = unsafe { GetProcAddress(module, c"GetDefaultKinectSensor".as_ptr().cast()) };
        let Some(proc) = proc else {
            return Err("Kinect20.dll does not export GetDefaultKinectSensor.".to_string());
        };
        let get_default_sensor = unsafe { std::mem::transmute::<FARPROC, GetDefaultKinectSensorFn>(Some(proc)) };

        let _ = module;

        Ok(Self { get_default_sensor })
    }
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct CameraSpacePointRaw {
    x: f32,
    y: f32,
    z: f32,
}

#[repr(C)]
#[derive(Clone, Copy, Default)]
struct JointRaw {
    joint_type: i32,
    position: CameraSpacePointRaw,
    tracking_state: i32,
}

#[repr(C)]
struct IKinectSensorVtbl {
    query_interface: Option<unsafe extern "system" fn(*mut IKinectSensor, *const GUID, *mut *mut c_void) -> HResult>,
    add_ref: Option<unsafe extern "system" fn(*mut IKinectSensor) -> ULong>,
    release: Option<unsafe extern "system" fn(*mut IKinectSensor) -> ULong>,
    subscribe_is_available_changed: usize,
    unsubscribe_is_available_changed: usize,
    get_is_available_changed_event_data: usize,
    open: Option<unsafe extern "system" fn(*mut IKinectSensor) -> HResult>,
    close: Option<unsafe extern "system" fn(*mut IKinectSensor) -> HResult>,
    get_is_open: usize,
    get_is_available: Option<unsafe extern "system" fn(*mut IKinectSensor, *mut Boolean) -> HResult>,
    get_color_frame_source: usize,
    get_depth_frame_source: usize,
    get_body_frame_source: Option<unsafe extern "system" fn(*mut IKinectSensor, *mut *mut IBodyFrameSource) -> HResult>,
    get_body_index_frame_source: usize,
    get_infrared_frame_source: usize,
    get_long_exposure_infrared_frame_source: usize,
    get_audio_source: usize,
    open_multi_source_frame_reader: usize,
    get_coordinate_mapper: usize,
    get_unique_kinect_id: usize,
    get_kinect_capabilities: usize,
}

#[repr(C)]
struct IKinectSensor {
    lp_vtbl: *const IKinectSensorVtbl,
}

#[repr(C)]
struct IBodyFrameSourceVtbl {
    query_interface: Option<unsafe extern "system" fn(*mut IBodyFrameSource, *const GUID, *mut *mut c_void) -> HResult>,
    add_ref: Option<unsafe extern "system" fn(*mut IBodyFrameSource) -> ULong>,
    release: Option<unsafe extern "system" fn(*mut IBodyFrameSource) -> ULong>,
    subscribe_frame_captured: usize,
    unsubscribe_frame_captured: usize,
    get_frame_captured_event_data: usize,
    get_is_active: usize,
    get_body_count: usize,
    open_reader: Option<unsafe extern "system" fn(*mut IBodyFrameSource, *mut *mut IBodyFrameReader) -> HResult>,
    get_kinect_sensor: usize,
    override_hand_tracking: usize,
    override_and_replace_hand_tracking: usize,
}

#[repr(C)]
struct IBodyFrameSource {
    lp_vtbl: *const IBodyFrameSourceVtbl,
}

#[repr(C)]
struct IBodyFrameReaderVtbl {
    query_interface: Option<unsafe extern "system" fn(*mut IBodyFrameReader, *const GUID, *mut *mut c_void) -> HResult>,
    add_ref: Option<unsafe extern "system" fn(*mut IBodyFrameReader) -> ULong>,
    release: Option<unsafe extern "system" fn(*mut IBodyFrameReader) -> ULong>,
    subscribe_frame_arrived: Option<unsafe extern "system" fn(*mut IBodyFrameReader, *mut WaitableHandle) -> HResult>,
    unsubscribe_frame_arrived: Option<unsafe extern "system" fn(*mut IBodyFrameReader, WaitableHandle) -> HResult>,
    get_frame_arrived_event_data: Option<
        unsafe extern "system" fn(
            *mut IBodyFrameReader,
            WaitableHandle,
            *mut *mut IBodyFrameArrivedEventArgs,
        ) -> HResult,
    >,
    acquire_latest_frame: usize,
    get_is_paused: usize,
    put_is_paused: usize,
    get_body_frame_source: usize,
}

#[repr(C)]
struct IBodyFrameReader {
    lp_vtbl: *const IBodyFrameReaderVtbl,
}

#[repr(C)]
struct IBodyFrameArrivedEventArgsVtbl {
    query_interface:
        Option<unsafe extern "system" fn(*mut IBodyFrameArrivedEventArgs, *const GUID, *mut *mut c_void) -> HResult>,
    add_ref: Option<unsafe extern "system" fn(*mut IBodyFrameArrivedEventArgs) -> ULong>,
    release: Option<unsafe extern "system" fn(*mut IBodyFrameArrivedEventArgs) -> ULong>,
    get_frame_reference:
        Option<unsafe extern "system" fn(*mut IBodyFrameArrivedEventArgs, *mut *mut IBodyFrameReference) -> HResult>,
}

#[repr(C)]
struct IBodyFrameArrivedEventArgs {
    lp_vtbl: *const IBodyFrameArrivedEventArgsVtbl,
}

#[repr(C)]
struct IBodyFrameReferenceVtbl {
    query_interface:
        Option<unsafe extern "system" fn(*mut IBodyFrameReference, *const GUID, *mut *mut c_void) -> HResult>,
    add_ref: Option<unsafe extern "system" fn(*mut IBodyFrameReference) -> ULong>,
    release: Option<unsafe extern "system" fn(*mut IBodyFrameReference) -> ULong>,
    acquire_frame: Option<unsafe extern "system" fn(*mut IBodyFrameReference, *mut *mut IBodyFrame) -> HResult>,
    get_relative_time: usize,
}

#[repr(C)]
struct IBodyFrameReference {
    lp_vtbl: *const IBodyFrameReferenceVtbl,
}

#[repr(C)]
struct IBodyFrameVtbl {
    query_interface: Option<unsafe extern "system" fn(*mut IBodyFrame, *const GUID, *mut *mut c_void) -> HResult>,
    add_ref: Option<unsafe extern "system" fn(*mut IBodyFrame) -> ULong>,
    release: Option<unsafe extern "system" fn(*mut IBodyFrame) -> ULong>,
    get_and_refresh_body_data: Option<unsafe extern "system" fn(*mut IBodyFrame, UInt, *mut *mut IBody) -> HResult>,
    get_floor_clip_plane: usize,
    get_relative_time: Option<unsafe extern "system" fn(*mut IBodyFrame, *mut Timespan) -> HResult>,
    get_body_frame_source: usize,
}

#[repr(C)]
struct IBodyFrame {
    lp_vtbl: *const IBodyFrameVtbl,
}

#[repr(C)]
struct IBodyVtbl {
    query_interface: Option<unsafe extern "system" fn(*mut IBody, *const GUID, *mut *mut c_void) -> HResult>,
    add_ref: Option<unsafe extern "system" fn(*mut IBody) -> ULong>,
    release: Option<unsafe extern "system" fn(*mut IBody) -> ULong>,
    get_joints: Option<unsafe extern "system" fn(*mut IBody, UInt, *mut JointRaw) -> HResult>,
    get_joint_orientations: usize,
    get_engaged: usize,
    get_expression_detection_results: usize,
    get_activity_detection_results: usize,
    get_appearance_detection_results: usize,
    get_hand_left_state: usize,
    get_hand_left_confidence: usize,
    get_hand_right_state: usize,
    get_hand_right_confidence: usize,
    get_clipped_edges: usize,
    get_tracking_id: Option<unsafe extern "system" fn(*mut IBody, *mut UInt64) -> HResult>,
    get_is_tracked: Option<unsafe extern "system" fn(*mut IBody, *mut Boolean) -> HResult>,
    get_is_restricted: usize,
    get_lean: usize,
    get_lean_tracking_state: usize,
}

#[repr(C)]
struct IBody {
    lp_vtbl: *const IBodyVtbl,
}

struct KinectSensorHandle {
    ptr: *mut IKinectSensor,
}

impl KinectSensorHandle {
    fn open_default(api: &KinectApi) -> Result<Self, String> {
        let mut ptr = ptr::null_mut();
        let hr = unsafe { (api.get_default_sensor)(&mut ptr) };
        check_hresult(hr, "GetDefaultKinectSensor")?;
        if ptr.is_null() {
            return Err("No Kinect 2 sensor is currently available.".to_string());
        }

        let sensor = Self { ptr };
        sensor.open()?;
        Ok(sensor)
    }

    fn open(&self) -> Result<(), String> {
        let vtbl = self.vtbl()?;
        let open = vtbl
            .open
            .ok_or_else(|| "Kinect 2 sensor does not expose Open.".to_string())?;
        let hr = unsafe { open(self.ptr) };
        check_hresult(hr, "IKinectSensor::Open")
    }

    fn close(&self) -> Result<(), String> {
        let vtbl = self.vtbl()?;
        let close = vtbl
            .close
            .ok_or_else(|| "Kinect 2 sensor does not expose Close.".to_string())?;
        let hr = unsafe { close(self.ptr) };
        check_hresult(hr, "IKinectSensor::Close")
    }

    fn is_available(&self) -> Result<bool, String> {
        let vtbl = self.vtbl()?;
        let get_is_available = vtbl
            .get_is_available
            .ok_or_else(|| "Kinect 2 sensor does not expose get_IsAvailable.".to_string())?;
        let mut value = 0u8;
        let hr = unsafe { get_is_available(self.ptr, &mut value) };
        check_hresult(hr, "IKinectSensor::get_IsAvailable")?;
        Ok(value != 0)
    }

    fn body_frame_source(&self) -> Result<BodyFrameSourceHandle, String> {
        let vtbl = self.vtbl()?;
        let get_body_frame_source = vtbl
            .get_body_frame_source
            .ok_or_else(|| "Kinect 2 sensor does not expose get_BodyFrameSource.".to_string())?;
        let mut ptr = ptr::null_mut();
        let hr = unsafe { get_body_frame_source(self.ptr, &mut ptr) };
        check_hresult(hr, "IKinectSensor::get_BodyFrameSource")?;
        if ptr.is_null() {
            return Err("Kinect 2 body-frame source returned null.".to_string());
        }
        Ok(BodyFrameSourceHandle { ptr })
    }

    fn vtbl(&self) -> Result<&IKinectSensorVtbl, String> {
        unsafe { self.ptr.as_ref() }
            .and_then(|sensor| unsafe { sensor.lp_vtbl.as_ref() })
            .ok_or_else(|| "Kinect 2 sensor pointer is null.".to_string())
    }
}

impl Drop for KinectSensorHandle {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }

        let _ = self.close();
        unsafe {
            if let Some(vtbl) = self.ptr.as_ref().and_then(|sensor| sensor.lp_vtbl.as_ref()) {
                if let Some(release) = vtbl.release {
                    release(self.ptr);
                }
            }
        }
        self.ptr = ptr::null_mut();
    }
}

struct BodyFrameSourceHandle {
    ptr: *mut IBodyFrameSource,
}

impl BodyFrameSourceHandle {
    fn open_reader(&self) -> Result<BodyFrameReaderHandle, String> {
        let vtbl = self.vtbl()?;
        let open_reader = vtbl
            .open_reader
            .ok_or_else(|| "Kinect 2 body-frame source does not expose OpenReader.".to_string())?;
        let mut ptr = ptr::null_mut();
        let hr = unsafe { open_reader(self.ptr, &mut ptr) };
        check_hresult(hr, "IBodyFrameSource::OpenReader")?;
        if ptr.is_null() {
            return Err("Kinect 2 body-frame reader returned null.".to_string());
        }
        Ok(BodyFrameReaderHandle { ptr })
    }

    fn vtbl(&self) -> Result<&IBodyFrameSourceVtbl, String> {
        unsafe { self.ptr.as_ref() }
            .and_then(|source| unsafe { source.lp_vtbl.as_ref() })
            .ok_or_else(|| "Kinect 2 body-frame source pointer is null.".to_string())
    }
}

impl Drop for BodyFrameSourceHandle {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }

        unsafe {
            if let Some(vtbl) = self.ptr.as_ref().and_then(|source| source.lp_vtbl.as_ref()) {
                if let Some(release) = vtbl.release {
                    release(self.ptr);
                }
            }
        }
        self.ptr = ptr::null_mut();
    }
}

struct BodyFrameReaderHandle {
    ptr: *mut IBodyFrameReader,
}

impl BodyFrameReaderHandle {
    fn subscribe_frame_arrived(&self, handle: &mut WaitableHandle) -> Result<(), String> {
        let vtbl = self.vtbl()?;
        let subscribe = vtbl
            .subscribe_frame_arrived
            .ok_or_else(|| "Kinect 2 body-frame reader does not expose SubscribeFrameArrived.".to_string())?;
        let hr = unsafe { subscribe(self.ptr, handle) };
        check_hresult(hr, "IBodyFrameReader::SubscribeFrameArrived")
    }

    fn unsubscribe_frame_arrived(&self, handle: WaitableHandle) -> Result<(), String> {
        let vtbl = self.vtbl()?;
        let unsubscribe = vtbl
            .unsubscribe_frame_arrived
            .ok_or_else(|| "Kinect 2 body-frame reader does not expose UnsubscribeFrameArrived.".to_string())?;
        let hr = unsafe { unsubscribe(self.ptr, handle) };
        check_hresult(hr, "IBodyFrameReader::UnsubscribeFrameArrived")
    }

    fn frame_arrived_event_data(&self, handle: WaitableHandle) -> Result<BodyFrameArrivedEventArgsHandle, String> {
        let vtbl = self.vtbl()?;
        let get_event_data = vtbl
            .get_frame_arrived_event_data
            .ok_or_else(|| "Kinect 2 body-frame reader does not expose GetFrameArrivedEventData.".to_string())?;
        let mut ptr = ptr::null_mut();
        let hr = unsafe { get_event_data(self.ptr, handle, &mut ptr) };
        check_hresult(hr, "IBodyFrameReader::GetFrameArrivedEventData")?;
        if ptr.is_null() {
            return Err("Kinect 2 body-frame event args returned null.".to_string());
        }
        Ok(BodyFrameArrivedEventArgsHandle { ptr })
    }

    fn vtbl(&self) -> Result<&IBodyFrameReaderVtbl, String> {
        unsafe { self.ptr.as_ref() }
            .and_then(|reader| unsafe { reader.lp_vtbl.as_ref() })
            .ok_or_else(|| "Kinect 2 body-frame reader pointer is null.".to_string())
    }
}

impl Drop for BodyFrameReaderHandle {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }

        unsafe {
            if let Some(vtbl) = self.ptr.as_ref().and_then(|reader| reader.lp_vtbl.as_ref()) {
                if let Some(release) = vtbl.release {
                    release(self.ptr);
                }
            }
        }
        self.ptr = ptr::null_mut();
    }
}

struct BodyFrameArrivedEventArgsHandle {
    ptr: *mut IBodyFrameArrivedEventArgs,
}

impl BodyFrameArrivedEventArgsHandle {
    fn frame_reference(&self) -> Result<BodyFrameReferenceHandle, String> {
        let vtbl = self.vtbl()?;
        let get_frame_reference = vtbl
            .get_frame_reference
            .ok_or_else(|| "Kinect 2 body-frame event args do not expose get_FrameReference.".to_string())?;
        let mut ptr = ptr::null_mut();
        let hr = unsafe { get_frame_reference(self.ptr, &mut ptr) };
        check_hresult(hr, "IBodyFrameArrivedEventArgs::get_FrameReference")?;
        if ptr.is_null() {
            return Err("Kinect 2 body-frame reference returned null.".to_string());
        }
        Ok(BodyFrameReferenceHandle { ptr })
    }

    fn vtbl(&self) -> Result<&IBodyFrameArrivedEventArgsVtbl, String> {
        unsafe { self.ptr.as_ref() }
            .and_then(|event_args| unsafe { event_args.lp_vtbl.as_ref() })
            .ok_or_else(|| "Kinect 2 body-frame event args pointer is null.".to_string())
    }
}

impl Drop for BodyFrameArrivedEventArgsHandle {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }

        unsafe {
            if let Some(vtbl) = self.ptr.as_ref().and_then(|event_args| event_args.lp_vtbl.as_ref()) {
                if let Some(release) = vtbl.release {
                    release(self.ptr);
                }
            }
        }
        self.ptr = ptr::null_mut();
    }
}

struct BodyFrameReferenceHandle {
    ptr: *mut IBodyFrameReference,
}

impl BodyFrameReferenceHandle {
    fn acquire_frame(&self) -> Result<BodyFrameHandle, String> {
        let vtbl = self.vtbl()?;
        let acquire_frame = vtbl
            .acquire_frame
            .ok_or_else(|| "Kinect 2 body-frame reference does not expose AcquireFrame.".to_string())?;
        let mut ptr = ptr::null_mut();
        let hr = unsafe { acquire_frame(self.ptr, &mut ptr) };
        check_hresult(hr, "IBodyFrameReference::AcquireFrame")?;
        if ptr.is_null() {
            return Err("Kinect 2 body-frame acquisition returned null.".to_string());
        }
        Ok(BodyFrameHandle { ptr })
    }

    fn vtbl(&self) -> Result<&IBodyFrameReferenceVtbl, String> {
        unsafe { self.ptr.as_ref() }
            .and_then(|reference| unsafe { reference.lp_vtbl.as_ref() })
            .ok_or_else(|| "Kinect 2 body-frame reference pointer is null.".to_string())
    }
}

impl Drop for BodyFrameReferenceHandle {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }

        unsafe {
            if let Some(vtbl) = self.ptr.as_ref().and_then(|reference| reference.lp_vtbl.as_ref()) {
                if let Some(release) = vtbl.release {
                    release(self.ptr);
                }
            }
        }
        self.ptr = ptr::null_mut();
    }
}

struct BodyFrameHandle {
    ptr: *mut IBodyFrame,
}

impl BodyFrameHandle {
    fn relative_time(&self) -> Result<Timespan, String> {
        let vtbl = self.vtbl()?;
        let get_relative_time = vtbl
            .get_relative_time
            .ok_or_else(|| "Kinect 2 body frame does not expose get_RelativeTime.".to_string())?;
        let mut value = 0;
        let hr = unsafe { get_relative_time(self.ptr, &mut value) };
        check_hresult(hr, "IBodyFrame::get_RelativeTime")?;
        Ok(value)
    }

    fn tracked_bodies(&self) -> Result<Vec<KinectBodySnapshot>, String> {
        let vtbl = self.vtbl()?;
        let get_and_refresh_body_data = vtbl
            .get_and_refresh_body_data
            .ok_or_else(|| "Kinect 2 body frame does not expose GetAndRefreshBodyData.".to_string())?;
        let mut body_ptrs = [ptr::null_mut(); BODY_COUNT];
        let hr = unsafe { get_and_refresh_body_data(self.ptr, BODY_COUNT as UInt, body_ptrs.as_mut_ptr()) };
        // Own every returned reference before any fallible body inspection.
        let bodies = body_ptrs.map(|ptr| BodyHandle { ptr });
        check_hresult(hr, "IBodyFrame::GetAndRefreshBodyData")?;

        collect_bodies(bodies)
    }

    fn vtbl(&self) -> Result<&IBodyFrameVtbl, String> {
        unsafe { self.ptr.as_ref() }
            .and_then(|frame| unsafe { frame.lp_vtbl.as_ref() })
            .ok_or_else(|| "Kinect 2 body-frame pointer is null.".to_string())
    }
}

impl Drop for BodyFrameHandle {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }

        unsafe {
            if let Some(vtbl) = self.ptr.as_ref().and_then(|frame| frame.lp_vtbl.as_ref()) {
                if let Some(release) = vtbl.release {
                    release(self.ptr);
                }
            }
        }
        self.ptr = ptr::null_mut();
    }
}

struct BodyHandle {
    ptr: *mut IBody,
}

impl BodyHandle {
    fn tracking_id(&self) -> Result<u64, String> {
        let vtbl = self.vtbl()?;
        let get_tracking_id = vtbl
            .get_tracking_id
            .ok_or_else(|| "Kinect 2 body does not expose get_TrackingId.".to_string())?;
        let mut value = 0;
        let hr = unsafe { get_tracking_id(self.ptr, &mut value) };
        check_hresult(hr, "IBody::get_TrackingId")?;
        Ok(value)
    }

    fn is_tracked(&self) -> Result<bool, String> {
        let vtbl = self.vtbl()?;
        let get_is_tracked = vtbl
            .get_is_tracked
            .ok_or_else(|| "Kinect 2 body does not expose get_IsTracked.".to_string())?;
        let mut value = 0u8;
        let hr = unsafe { get_is_tracked(self.ptr, &mut value) };
        check_hresult(hr, "IBody::get_IsTracked")?;
        Ok(value != 0)
    }

    fn joints(&self) -> Result<[JointRaw; KinectJoint::COUNT], String> {
        let vtbl = self.vtbl()?;
        let get_joints = vtbl
            .get_joints
            .ok_or_else(|| "Kinect 2 body does not expose GetJoints.".to_string())?;
        let mut joints = [JointRaw::default(); KinectJoint::COUNT];
        let hr = unsafe { get_joints(self.ptr, KinectJoint::COUNT as UInt, joints.as_mut_ptr()) };
        check_hresult(hr, "IBody::GetJoints")?;
        Ok(joints)
    }

    fn snapshot(&self) -> Result<KinectBodySnapshot, String> {
        Ok(KinectBodySnapshot {
            tracking_id: self.tracking_id()?,
            joints: decode_joints(self.joints()?)?,
        })
    }

    fn vtbl(&self) -> Result<&IBodyVtbl, String> {
        unsafe { self.ptr.as_ref() }
            .and_then(|body| unsafe { body.lp_vtbl.as_ref() })
            .ok_or_else(|| "Kinect 2 body pointer is null.".to_string())
    }
}

impl Drop for BodyHandle {
    fn drop(&mut self) {
        if self.ptr.is_null() {
            return;
        }

        unsafe {
            if let Some(vtbl) = self.ptr.as_ref().and_then(|body| body.lp_vtbl.as_ref()) {
                if let Some(release) = vtbl.release {
                    release(self.ptr);
                }
            }
        }
        self.ptr = ptr::null_mut();
    }
}

fn collect_bodies(bodies: [BodyHandle; BODY_COUNT]) -> Result<Vec<KinectBodySnapshot>, String> {
    let mut tracked = Vec::with_capacity(BODY_COUNT);
    for body in bodies.into_iter().filter(|body| !body.ptr.is_null()) {
        if body.is_tracked()? {
            tracked.push(body.snapshot()?);
        }
    }
    Ok(tracked)
}
fn decode_joints(joints: [JointRaw; KinectJoint::COUNT]) -> Result<[KinectJointSample; KinectJoint::COUNT], String> {
    let mut samples = [KinectJointSample::default(); KinectJoint::COUNT];
    let mut seen = [false; KinectJoint::COUNT];
    for joint in joints {
        let kind = KinectJoint::from_raw(joint.joint_type).ok_or("Kinect joint identity is invalid")?;
        if std::mem::replace(&mut seen[kind.index()], true) {
            return Err("Kinect joint identity is duplicated".into());
        }
        let position = KinectVec3::new(
            f64::from(joint.position.x),
            f64::from(joint.position.y),
            f64::from(joint.position.z),
        );
        if !position.x.is_finite() || !position.y.is_finite() || !position.z.is_finite() {
            return Err("Kinect joint position is not finite".into());
        }
        let tracking_state = match joint.tracking_state {
            0 => KinectTrackingState::NotTracked,
            1 => KinectTrackingState::Inferred,
            2 => KinectTrackingState::Tracked,
            _ => return Err("Kinect joint tracking state is invalid".into()),
        };
        samples[kind.index()] = KinectJointSample {
            position,
            tracking_state,
        };
    }
    Ok(samples)
}
fn frame_timestamp(raw: Timespan) -> Result<u64, String> {
    u64::try_from(raw).map_err(|_| "Kinect frame timestamp is negative".into())
}

fn wide_null(value: &str) -> Vec<u16> {
    OsStr::new(value).encode_wide().chain(std::iter::once(0)).collect()
}

fn check_hresult(result: HResult, context: &str) -> Result<(), String> {
    if result >= 0 {
        Ok(())
    } else {
        Err(format!("{context} failed with HRESULT 0x{:08X}", result as u32))
    }
}

#[cfg(test)]
mod tests;
