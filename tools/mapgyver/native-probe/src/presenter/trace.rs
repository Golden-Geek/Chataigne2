//! Ephemeral CLI diagnostics for matching this probe's Present calls to ETW records.
//! These addresses are identifiers only; no native interface or window is exported to the UI.

use crate::{ProbeResult, clock};
use serde::Serialize;
use windows::Win32::Graphics::Dxgi::{IDXGISwapChain, IDXGISwapChain1, IDXGISwapChain2};
use windows::core::Interface;

#[derive(Clone, Copy, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Role {
    Editor,
    NativeOutput,
}

#[derive(Clone, Serialize)]
pub struct Addresses {
    pub idxgi_swap_chain: String,
    pub idxgi_swap_chain1: String,
    pub idxgi_swap_chain2: String,
}

pub struct Identity {
    role: Role,
    begin_qpc: u64,
    addresses: Addresses,
}

#[derive(Serialize)]
pub struct Report {
    pub role: Role,
    // Encloses every Present call, from before construction until after the final call.
    // It is not a measurement of COM allocation or physical display lifetime.
    pub begin_qpc: u64,
    pub end_qpc: u64,
    pub addresses: Addresses,
}

impl Identity {
    pub fn new(chain: &IDXGISwapChain2, role: Role, begin_qpc: u64) -> ProbeResult<Self> {
        // Query each interface instead of assuming all COM aliases share an address.
        // The temporary references retire here; the diagnostic retains only hex strings.
        let base = chain.cast::<IDXGISwapChain>()?;
        let version1 = chain.cast::<IDXGISwapChain1>()?;
        Ok(Self {
            role,
            begin_qpc,
            addresses: Addresses {
                idxgi_swap_chain: format!("{:#x}", base.as_raw() as usize),
                idxgi_swap_chain1: format!("{:#x}", version1.as_raw() as usize),
                idxgi_swap_chain2: format!("{:#x}", chain.as_raw() as usize),
            },
        })
    }

    pub fn finish(&self) -> ProbeResult<Report> {
        Ok(Report {
            role: self.role,
            begin_qpc: self.begin_qpc,
            end_qpc: clock::now()?,
            addresses: self.addresses.clone(),
        })
    }
}
