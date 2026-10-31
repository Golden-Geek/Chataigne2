use super::*;
use ::windows::Win32::Globalization::{CSTR_GREATER_THAN, CSTR_LESS_THAN, CompareStringOrdinal};
use std::{cmp::Ordering, ffi::OsStr, os::windows::ffi::OsStrExt};

const MAX_UTF16: usize = 32_767;
pub(super) struct Prepared {
    pub executable: Vec<u16>,
    pub command: Vec<u16>,
    pub directory: Option<Vec<u16>>,
    pub environment: Option<Vec<u16>>,
}
fn invalid(message: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidInput, message)
}
fn wide(value: &OsStr) -> io::Result<Vec<u16>> {
    let value = value.encode_wide().take(MAX_UTF16).collect::<Vec<_>>();
    if value.contains(&0) || value.len() >= MAX_UTF16 {
        return Err(invalid("process strings must be NUL-free and below 32767 UTF-16 units"));
    }
    Ok(value)
}
fn quote(value: &[u16], output: &mut Vec<u16>) {
    output.push(b'"' as u16);
    let mut slashes = 0;
    for &unit in value {
        if unit == b'\\' as u16 {
            slashes += 1;
            continue;
        }
        let count = if unit == b'"' as u16 { slashes * 2 + 1 } else { slashes };
        output.extend(std::iter::repeat_n(b'\\' as u16, count));
        output.push(unit);
        slashes = 0;
    }
    output.extend(std::iter::repeat_n(b'\\' as u16, slashes * 2));
    output.push(b'"' as u16);
}
impl Prepared {
    pub fn new(spec: &ProcessSpec) -> io::Result<Self> {
        if !spec.executable.is_absolute()
            || !spec
                .executable
                .extension()
                .is_some_and(|extension| extension.eq_ignore_ascii_case("exe"))
        {
            return Err(invalid(
                "owned processes require an absolute .exe path; no shell or executable lookup is implicit",
            ));
        }
        let mut executable = wide(spec.executable.as_os_str())?;
        if executable.contains(&(b'"' as u16)) {
            return Err(invalid("executable path cannot contain quotes"));
        }
        let mut command = vec![];
        quote(&executable, &mut command);
        for argument in &spec.arguments {
            command.push(b' ' as u16);
            quote(&wide(argument)?, &mut command);
            if command.len() >= MAX_UTF16 {
                return Err(invalid("process command line exceeds 32767 UTF-16 units"));
            }
        }
        if command.len() >= MAX_UTF16 {
            return Err(invalid("process command line exceeds 32767 UTF-16 units"));
        }
        executable.push(0);
        command.push(0);
        let directory = spec
            .directory
            .as_ref()
            .map(|directory| {
                if !directory.is_absolute() {
                    return Err(invalid("process directory must be absolute"));
                }
                let mut value = wide(directory.as_os_str())?;
                value.push(0);
                Ok(value)
            })
            .transpose()?;
        let environment = spec
            .environment
            .as_ref()
            .map(|entries| {
                let mut prepared = Vec::new();
                let mut units = 1usize;
                for (name, value) in entries {
                    let name = wide(name)?;
                    if name.is_empty() || name.contains(&(b'=' as u16)) {
                        return Err(invalid("process environment names must be nonempty and contain no '='"));
                    }
                    let value = wide(value)?;
                    units = units
                        .checked_add(name.len() + value.len() + 2)
                        .ok_or_else(|| invalid("process environment size overflow"))?;
                    if units > MAX_UTF16 {
                        return Err(invalid("explicit process environment exceeds 32767 UTF-16 units"));
                    }
                    prepared.push((name, value));
                }
                let mut entries = prepared;
                entries.sort_by(|left, right| compare(&left.0, &right.0));
                if entries
                    .windows(2)
                    .any(|pair| compare(&pair[0].0, &pair[1].0) == Ordering::Equal)
                {
                    return Err(invalid("process environment contains duplicate case-insensitive names"));
                }
                let mut block = vec![];
                for (name, value) in entries {
                    block.extend(name);
                    block.push(b'=' as u16);
                    block.extend(value);
                    block.push(0);
                    if block.len() >= MAX_UTF16 {
                        return Err(invalid("explicit process environment exceeds 32767 UTF-16 units"));
                    }
                }
                if block.is_empty() {
                    block.push(0);
                }
                block.push(0);
                Ok(block)
            })
            .transpose()?;
        Ok(Self {
            executable,
            command,
            directory,
            environment,
        })
    }
}
fn compare(left: &[u16], right: &[u16]) -> Ordering {
    // Lengths are admitted above; Windows ordinal case folding matches environment-name identity.
    match unsafe { CompareStringOrdinal(left, right, true) } {
        CSTR_LESS_THAN => Ordering::Less,
        CSTR_GREATER_THAN => Ordering::Greater,
        _ => Ordering::Equal,
    }
}

pub(super) struct Attributes {
    _storage: Vec<usize>,
    _job: Box<HANDLE>,
    list: LPPROC_THREAD_ATTRIBUTE_LIST,
}
impl Attributes {
    pub fn new(job: HANDLE) -> io::Result<Self> {
        let mut bytes = 0;
        let _ = unsafe { InitializeProcThreadAttributeList(None, 1, None, &mut bytes) };
        if bytes == 0 || bytes > 65_536 {
            return Err(io::Error::other("invalid process attribute buffer size"));
        }
        let mut storage = vec![0usize; bytes.div_ceil(size_of::<usize>())];
        let list = LPPROC_THREAD_ATTRIBUTE_LIST(storage.as_mut_ptr().cast());
        unsafe {
            InitializeProcThreadAttributeList(Some(list), 1, None, &mut bytes).map_err(os_error)?;
        }
        let value = Self {
            _storage: storage,
            _job: Box::new(job),
            list,
        };
        // Boxed value and backing allocation remain stable through CreateProcess and list deletion.
        unsafe {
            UpdateProcThreadAttribute(
                value.list,
                0,
                PROC_THREAD_ATTRIBUTE_JOB_LIST as usize,
                Some((&*value._job as *const HANDLE).cast()),
                size_of::<HANDLE>(),
                None,
                None,
            )
            .map_err(os_error)?;
        }
        Ok(value)
    }
    pub fn list(&self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.list
    }
}
impl Drop for Attributes {
    fn drop(&mut self) {
        unsafe {
            DeleteProcThreadAttributeList(self.list);
        }
    }
}
