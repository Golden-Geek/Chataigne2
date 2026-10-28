use windows::Win32::Graphics::Direct3D11::{
    D3D11_ASYNC_GETDATA_DONOTFLUSH, D3D11_QUERY_DESC, D3D11_QUERY_EVENT, ID3D11Query,
};
use windows::core::{BOOL, Error, HRESULT, Result};

use crate::runtime::{CompletionToken, DeviceGeneration};

use super::device::Device;

struct Slot {
    query: ID3D11Query,
    token: Option<CompletionToken>,
}

/// Queries are allocated before any draw. End/Flush cannot abandon a partially submitted use.
pub(super) struct EventCompletion {
    slots: [Slot; 2],
    completed: u64,
}

pub(super) struct Submission<'a> {
    completion: &'a mut EventCompletion,
    device: &'a Device,
    token: CompletionToken,
}

impl Drop for Submission<'_> {
    fn drop(&mut self) {
        self.completion.mark(self.device, self.token);
    }
}

impl EventCompletion {
    pub(super) fn new(device: &Device) -> Result<Self> {
        let query = || -> Result<Slot> {
            let mut query = None;
            // SAFETY: a fixed completion event, created on the owning device before submission.
            unsafe {
                device.device.CreateQuery(
                    &D3D11_QUERY_DESC {
                        Query: D3D11_QUERY_EVENT,
                        MiscFlags: 0,
                    },
                    Some(&mut query),
                )?;
            }
            Ok(Slot {
                query: query.ok_or_else(|| Error::from_hresult(HRESULT(0x8000_4005_u32 as i32)))?,
                token: None,
            })
        };
        Ok(Self {
            slots: [query()?, query()?],
            completed: 0,
        })
    }

    pub(super) fn can_submit(&self) -> bool {
        self.slots.iter().any(|slot| slot.token.is_none())
    }

    pub(super) fn begin<'a>(&'a mut self, device: &'a Device, token: CompletionToken) -> Submission<'a> {
        // Preflight runs before this guard is created and before the first GPU command.
        Submission {
            completion: self,
            device,
            token,
        }
    }

    pub(super) fn mark(&mut self, device: &Device, token: CompletionToken) {
        let slot = self
            .slots
            .iter_mut()
            .find(|slot| slot.token.is_none())
            .expect("preflighted completion slot");
        slot.token = Some(token);
        // SAFETY: this event follows every queued draw/output operation on the same immediate
        // context. Flush submits it before the worker sleeps; polling never forces a flush.
        unsafe {
            device.context.End(&slot.query);
            device.context.Flush();
        }
    }

    pub(super) fn completed(&mut self, device: &Device, generation: DeviceGeneration) -> Result<CompletionToken> {
        while let Some(index) = self
            .slots
            .iter()
            .enumerate()
            .filter_map(|(index, slot)| slot.token.map(|token| (index, token.serial)))
            .min_by_key(|(_, serial)| *serial)
            .map(|(index, _)| index)
        {
            let slot = &mut self.slots[index];
            let mut done = BOOL(0);
            // SAFETY: the event's output is a BOOL. S_FALSE is a successful HRESULT but does
            // not write completion; the initialized FALSE must not be treated as finished.
            unsafe {
                device.context.GetData(
                    &slot.query,
                    Some((&mut done as *mut BOOL).cast()),
                    size_of::<BOOL>() as u32,
                    D3D11_ASYNC_GETDATA_DONOTFLUSH.0 as u32,
                )?;
            }
            if !done.as_bool() {
                break;
            }
            self.completed = slot.token.take().unwrap().serial;
        }
        Ok(CompletionToken {
            device: generation,
            serial: self.completed,
        })
    }
}
