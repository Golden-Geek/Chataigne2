use super::OwnedProcess;
use std::{fmt, io};

/// A creation failure may already own a target. That owner permits retirement, never resume.
/// Keep it until completion is verified; dropping the error requests termination without proving it.
pub struct CreateError<P = OwnedProcess> {
    error: io::Error,
    owner: Option<Box<P>>,
}
impl<P> CreateError<P> {
    pub fn error(&self) -> &io::Error {
        &self.error
    }
    pub fn into_parts(self) -> (io::Error, Option<P>) {
        (self.error, self.owner.map(|owner| *owner))
    }
    #[cfg(any(windows, test))]
    pub(super) fn with_owner(error: io::Error, owner: P) -> Self {
        Self {
            error,
            owner: Some(Box::new(owner)),
        }
    }
    #[cfg(windows)]
    pub(super) fn owner(&self) -> Option<&P> {
        self.owner.as_deref()
    }
    #[cfg(windows)]
    pub(super) fn owner_mut(&mut self) -> Option<&mut P> {
        self.owner.as_deref_mut()
    }
    pub(super) fn map_owner<Q>(self, map: impl FnOnce(P) -> Q) -> CreateError<Q> {
        CreateError {
            error: self.error,
            owner: self.owner.map(|owner| Box::new(map(*owner))),
        }
    }
}
impl<P> From<io::Error> for CreateError<P> {
    fn from(error: io::Error) -> Self {
        Self { error, owner: None }
    }
}
impl<P> fmt::Debug for CreateError<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("CreateError")
            .field("error", &self.error)
            .field("owner_retained", &self.owner.is_some())
            .finish()
    }
}
impl<P> fmt::Display for CreateError<P> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}
impl<P> std::error::Error for CreateError<P> {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&self.error)
    }
}

#[cfg(test)]
mod tests;
