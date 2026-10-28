//! Preserve GetData's S_FALSE readiness result; the generated Result<()> wrapper erases it.

use windows::Win32::Foundation::S_OK;
#[cfg(feature = "desktop")]
use windows::Win32::Graphics::Direct3D11::D3D11_QUERY_DATA_TIMESTAMP_DISJOINT;
use windows::Win32::Graphics::Direct3D11::{D3D11_ASYNC_GETDATA_DONOTFLUSH, ID3D11DeviceContext, ID3D11Query};
use windows::core::Interface;

use crate::ProbeResult;

unsafe fn read<T: Default>(context: &ID3D11DeviceContext, query: &ID3D11Query) -> ProbeResult<Option<T>> {
    let mut data = T::default();
    // SAFETY: callers select the ABI data structure matching this query kind. These public
    // generated vtable bindings retain the HRESULT without declaring a second Windows ABI.
    let result = unsafe {
        (context.vtable().GetData)(
            context.as_raw(),
            query.as_raw(),
            (&mut data as *mut T).cast(),
            std::mem::size_of::<T>() as u32,
            D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
        )
    };
    result.ok()?;
    Ok((result == S_OK).then_some(data))
}

pub fn event(context: &ID3D11DeviceContext, query: &ID3D11Query) -> ProbeResult<bool> {
    // SAFETY: this entry is used only for D3D11_QUERY_EVENT (BOOL/u32 result).
    Ok(unsafe { read::<u32>(context, query)? }.is_some_and(|ready| ready != 0))
}

#[cfg(feature = "desktop")]
pub fn timestamp(context: &ID3D11DeviceContext, query: &ID3D11Query) -> ProbeResult<Option<u64>> {
    // SAFETY: caller passes a D3D11_QUERY_TIMESTAMP (UINT64 result).
    unsafe { read(context, query) }
}

#[cfg(feature = "desktop")]
pub fn clock(
    context: &ID3D11DeviceContext,
    query: &ID3D11Query,
) -> ProbeResult<Option<D3D11_QUERY_DATA_TIMESTAMP_DISJOINT>> {
    // SAFETY: caller passes a D3D11_QUERY_TIMESTAMP_DISJOINT with this exact result type.
    unsafe { read(context, query) }
}
