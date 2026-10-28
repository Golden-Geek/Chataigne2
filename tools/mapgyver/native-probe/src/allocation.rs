//! Logical texture ownership on one worker. Completion markers bound pending retirement;
//! these counters never claim physical residency or private driver allocation visibility.

use serde::{Deserialize, Serialize};
use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

pub const OWNERS: [&str; 4] = ["application", "swapchain", "sdk_receiver_copy", "sdk_sender_shared"];

#[derive(Clone, Copy)]
pub enum Owner {
    Application,
    Swapchain,
    SdkReceiverCopy,
    SdkSenderShared,
}

#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Snapshot {
    pub live_bytes_by_owner: [u64; 4],
    pub retiring_bytes_by_owner: [u64; 4],
    pub live_textures: u64,
    pub retiring_textures: u64,
    pub peak_live_plus_retiring_bytes: u64,
    pub peak_live_plus_retiring_textures: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Operations {
    pub clears: u64,
    pub shader_draws: u64,
    pub uniform_upload_bytes: u64,
    pub diagnostic_readback_pixels: u64,
    pub sdk_send_calls: u64,
    pub sdk_receive_calls: u64,
    pub sdk_texture_queries: u64,
    pub sdk_texture_observations: u64,
    pub explicit_gpu_copy_calls: u64,
    pub explicit_gpu_copy_bytes: u64,
}

pub enum Work {
    Clear,
    Draw,
    UniformUpload(u64),
    Readback(u64),
    SdkSend,
    SdkReceive,
    SdkTextureQuery,
    SdkObservation,
    GpuCopy(u64),
}

#[derive(Default)]
struct State {
    snapshot: Snapshot,
    operations: Operations,
    submitted: u64,
    completed: u64,
    retiring: VecDeque<(u64, Owner, u64, u64)>,
}

#[derive(Clone, Default)]
pub struct Ledger(Rc<RefCell<State>>);

/// Cloning a lease retains one allocation; it does not count another physical texture.
#[derive(Clone)]
pub struct Lease {
    _allocation: Rc<Allocation>,
}

struct Allocation {
    ledger: Ledger,
    owner: Owner,
    bytes: u64,
    count: u64,
}

pub fn bgra_bytes(width: u32, height: u32, count: u32) -> Result<u64, String> {
    if width == 0 || height == 0 || count == 0 {
        return Err("empty logical texture extent".into());
    }
    u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|bytes| bytes.checked_mul(4))
        .and_then(|bytes| bytes.checked_mul(u64::from(count)))
        .ok_or_else(|| "logical BGRA allocation overflow".into())
}

impl Ledger {
    pub fn allocate(&self, owner: Owner, bytes: u64, count: u64) -> Result<Lease, String> {
        if bytes == 0 || count == 0 {
            return Err("empty logical allocation".into());
        }
        let mut state = self.0.borrow_mut();
        let snapshot = &mut state.snapshot;
        let total_bytes = snapshot
            .live_bytes_by_owner
            .iter()
            .chain(&snapshot.retiring_bytes_by_owner)
            .try_fold(bytes, |sum, value| sum.checked_add(*value))
            .ok_or("logical allocation total overflow")?;
        let total_count = snapshot
            .live_textures
            .checked_add(snapshot.retiring_textures)
            .and_then(|total| total.checked_add(count))
            .ok_or("logical texture count overflow")?;
        snapshot.live_bytes_by_owner[owner as usize] += bytes;
        snapshot.live_textures += count;
        snapshot.peak_live_plus_retiring_bytes = snapshot.peak_live_plus_retiring_bytes.max(total_bytes);
        snapshot.peak_live_plus_retiring_textures = snapshot.peak_live_plus_retiring_textures.max(total_count);
        Ok(Lease {
            _allocation: Rc::new(Allocation {
                ledger: self.clone(),
                owner,
                bytes,
                count,
            }),
        })
    }

    pub fn work(&self, work: Work) {
        let mut state = self.0.borrow_mut();
        // Every probe process has a bounded <=216,120-frame lifetime, far below u64 limits.
        state.submitted += 1;
        match work {
            Work::Clear => state.operations.clears += 1,
            Work::Draw => state.operations.shader_draws += 1,
            Work::UniformUpload(bytes) => state.operations.uniform_upload_bytes += bytes,
            Work::Readback(pixels) => state.operations.diagnostic_readback_pixels += pixels,
            Work::SdkSend => state.operations.sdk_send_calls += 1,
            Work::SdkReceive => state.operations.sdk_receive_calls += 1,
            Work::SdkTextureQuery => state.operations.sdk_texture_queries += 1,
            Work::SdkObservation => state.operations.sdk_texture_observations += 1,
            Work::GpuCopy(bytes) => {
                state.operations.explicit_gpu_copy_calls += 1;
                state.operations.explicit_gpu_copy_bytes += bytes;
            }
        }
    }

    pub fn submitted(&self) -> u64 {
        self.0.borrow().submitted
    }

    pub fn complete(&self, serial: u64) {
        let mut state = self.0.borrow_mut();
        assert!(serial <= state.submitted, "completion exceeds submitted work");
        state.completed = state.completed.max(serial);
        while let Some(&(retirement, owner, bytes, count)) = state.retiring.front() {
            if retirement > state.completed {
                break;
            }
            state.retiring.pop_front();
            state.snapshot.retiring_bytes_by_owner[owner as usize] -= bytes;
            state.snapshot.retiring_textures -= count;
        }
    }

    pub fn snapshot(&self) -> Snapshot {
        self.0.borrow().snapshot.clone()
    }
    pub fn operations(&self) -> Operations {
        self.0.borrow().operations.clone()
    }
}

impl Drop for Allocation {
    fn drop(&mut self) {
        let mut state = self.ledger.0.borrow_mut();
        state.snapshot.live_bytes_by_owner[self.owner as usize] -= self.bytes;
        state.snapshot.live_textures -= self.count;
        let serial = state.submitted;
        if serial > state.completed {
            state.snapshot.retiring_bytes_by_owner[self.owner as usize] += self.bytes;
            state.snapshot.retiring_textures += self.count;
            if let Some((last, owner, bytes, count)) = state.retiring.back_mut()
                && *last == serial
                && *owner as usize == self.owner as usize
            {
                *bytes += self.bytes;
                *count += self.count;
            } else {
                state.retiring.push_back((serial, self.owner, self.bytes, self.count));
            }
        }
    }
}

#[cfg(test)]
mod tests;
