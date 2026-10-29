use windows::Win32::Foundation::S_OK;
use windows::Win32::Graphics::Direct3D11::{
    D3D11_ASYNC_GETDATA_DONOTFLUSH, D3D11_QUERY, D3D11_QUERY_DATA_TIMESTAMP_DISJOINT, D3D11_QUERY_DESC,
    D3D11_QUERY_TIMESTAMP, D3D11_QUERY_TIMESTAMP_DISJOINT, ID3D11Device, ID3D11DeviceContext, ID3D11Query,
};
use windows::core::{Error, HRESULT, Interface, Result};

use super::{DiagnosticContext, DiagnosticEvent, GpuSampleStatus, GpuTrace, Recorder};
use crate::runtime::CompletionToken;

struct Slot {
    disjoint: ID3D11Query,
    begin: ID3D11Query,
    end: ID3D11Query,
    pending: Option<CompletionToken>,
}

/// Two context-local query slots. No extra event/flush, admission dependency or blocking read.
pub(crate) struct GpuTiming {
    slots: Option<[Slot; 2]>,
    failure: Option<HRESULT>,
    recorder: Recorder,
    context: DiagnosticContext,
}

/// End diagnostic markers before the caller's existing completion End/Flush, including unwind.
pub(crate) struct GpuScope<'a> {
    owner: &'a mut GpuTiming,
    context: &'a ID3D11DeviceContext,
    active: Option<usize>,
}

impl Drop for GpuScope<'_> {
    fn drop(&mut self) {
        if let Some(index) = self.active {
            let slot = &self.owner.slots.as_ref().unwrap()[index];
            // SAFETY: this scope owns the only active disjoint on the immediate context.
            unsafe {
                self.context.End(&slot.end);
                self.context.End(&slot.disjoint);
            }
        }
    }
}

fn query(device: &ID3D11Device, kind: D3D11_QUERY) -> Result<ID3D11Query> {
    let mut result = None;
    // SAFETY: query allocation precedes commands; the owned result remains on this worker.
    unsafe {
        device.CreateQuery(
            &D3D11_QUERY_DESC {
                Query: kind,
                MiscFlags: 0,
            },
            Some(&mut result),
        )?;
    }
    result.ok_or_else(|| Error::from_hresult(HRESULT(0x80004005_u32 as i32)))
}

unsafe fn read<T: Default>(context: &ID3D11DeviceContext, query: &ID3D11Query) -> Result<Option<T>> {
    let mut value = T::default();
    // SAFETY: each caller selects the exact query payload. Use the generated public vtable:
    // Result<()> erases S_FALSE, which must never fabricate an available zero timestamp.
    let result = unsafe {
        (context.vtable().GetData)(
            context.as_raw(),
            query.as_raw(),
            (&mut value as *mut T).cast(),
            size_of::<T>() as u32,
            D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
        )
    };
    available(result, value)
}

pub(super) fn available<T>(result: HRESULT, value: T) -> Result<Option<T>> {
    result.ok()?;
    Ok((result == S_OK).then_some(value))
}

pub(super) fn take_completed(
    pending: &mut Option<CompletionToken>,
    completed: CompletionToken,
) -> Option<CompletionToken> {
    if pending.is_some_and(|token| token.device == completed.device && token.serial <= completed.serial) {
        pending.take()
    } else {
        None
    }
}

pub(super) fn validity(begin: u64, end: u64, frequency: u64, disjoint: bool) -> GpuSampleStatus {
    if disjoint {
        GpuSampleStatus::Disjoint
    } else if frequency == 0 || end < begin {
        GpuSampleStatus::Invalid
    } else {
        GpuSampleStatus::Valid
    }
}

impl GpuTiming {
    pub(crate) fn new(device: &ID3D11Device, recorder: Recorder, context: DiagnosticContext) -> Self {
        let create = || {
            Ok::<_, Error>(Slot {
                disjoint: query(device, D3D11_QUERY_TIMESTAMP_DISJOINT)?,
                begin: query(device, D3D11_QUERY_TIMESTAMP)?,
                end: query(device, D3D11_QUERY_TIMESTAMP)?,
                pending: None,
            })
        };
        let slots = (|| Ok::<_, Error>([create()?, create()?]))();
        match slots {
            Ok(slots) => Self {
                slots: Some(slots),
                failure: None,
                recorder,
                context,
            },
            Err(error) => Self {
                slots: None,
                failure: Some(error.code()),
                recorder,
                context,
            },
        }
    }

    fn unavailable(&self, token: CompletionToken, status: GpuSampleStatus) {
        self.recorder.push(DiagnosticEvent::Gpu(GpuTrace {
            token,
            context: self.context,
            status,
            begin: None,
            end: None,
            frequency: None,
        }));
    }

    pub(crate) fn begin<'a>(&'a mut self, context: &'a ID3D11DeviceContext, token: CompletionToken) -> GpuScope<'a> {
        let active = self
            .slots
            .as_ref()
            .and_then(|slots| slots.iter().position(|slot| slot.pending.is_none()));
        if let Some(index) = active {
            let slot = &mut self.slots.as_mut().unwrap()[index];
            slot.pending = Some(token);
            // SAFETY: the slot is unused and exactly one disjoint scope is active at a time.
            unsafe {
                context.Begin(&slot.disjoint);
                context.End(&slot.begin);
            }
        } else {
            let status = self
                .failure
                .map_or(GpuSampleStatus::PoolBusy, |error| GpuSampleStatus::Failed {
                    hresult: error.0,
                });
            self.unavailable(token, status);
        }
        GpuScope {
            owner: self,
            context,
            active,
        }
    }

    pub(crate) fn poll(&mut self, context: &ID3D11DeviceContext, completed: CompletionToken) {
        let Some(slots) = &mut self.slots else {
            return;
        };
        for slot in slots {
            let Some(token) = slot
                .pending
                .filter(|token| token.device == completed.device && token.serial <= completed.serial)
            else {
                continue;
            };
            let result = (|| {
                // SAFETY: these queries were created with their matching result types.
                let Some(clock) = unsafe { read::<D3D11_QUERY_DATA_TIMESTAMP_DISJOINT>(context, &slot.disjoint) }?
                else {
                    return Ok(None);
                };
                let Some(begin) = unsafe { read::<u64>(context, &slot.begin) }? else {
                    return Ok(None);
                };
                let Some(end) = unsafe { read::<u64>(context, &slot.end) }? else {
                    return Ok(None);
                };
                Ok::<_, Error>(Some((begin, end, clock)))
            })();
            let row = match result {
                Ok(None) => continue,
                Ok(Some((begin, end, clock))) => GpuTrace {
                    token,
                    context: self.context,
                    status: validity(begin, end, clock.Frequency, clock.Disjoint.as_bool()),
                    begin: Some(begin),
                    end: Some(end),
                    frequency: Some(clock.Frequency),
                },
                Err(error) => GpuTrace {
                    token,
                    context: self.context,
                    status: GpuSampleStatus::Failed {
                        hresult: error.code().0,
                    },
                    begin: None,
                    end: None,
                    frequency: None,
                },
            };
            // The real event completed this batch, so neither a failed read nor slot reuse
            // can release/rewrite a query still used by queued GPU commands.
            slot.pending = None;
            self.recorder.push(DiagnosticEvent::Gpu(row));
        }
    }
}

impl GpuTiming {
    /// Final nonblocking collection for batches already covered by actual completion.
    /// Remaining S_FALSE results become explicit Uncollected rows before the caller's report.
    pub(crate) fn finish(&mut self, context: &ID3D11DeviceContext, completed: CompletionToken) {
        self.poll(context, completed);
        if let Some(slots) = &mut self.slots {
            for slot in slots {
                if let Some(token) = take_completed(&mut slot.pending, completed) {
                    self.recorder.push(DiagnosticEvent::Gpu(GpuTrace {
                        token,
                        context: self.context,
                        status: GpuSampleStatus::Uncollected,
                        begin: None,
                        end: None,
                        frequency: None,
                    }));
                }
            }
        }
    }

    /// Reports missing telemetry at terminal owner cleanup/removal. This grants no native
    /// resource-retirement authority and does not replace a real completion fence.
    pub(crate) fn abandon(&mut self) {
        if let Some(slots) = &mut self.slots {
            for slot in slots {
                if let Some(token) = slot.pending.take() {
                    self.recorder.push(DiagnosticEvent::Gpu(GpuTrace {
                        token,
                        context: self.context,
                        status: GpuSampleStatus::Uncollected,
                        begin: None,
                        end: None,
                        frequency: None,
                    }));
                }
            }
        }
    }
}

impl Drop for GpuTiming {
    fn drop(&mut self) {
        self.abandon();
    }
}
