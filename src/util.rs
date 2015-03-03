// The MIT License (MIT)
//
// Copyright (c) 2015 Nathan Sizemore <nathanrsizemore@gmail.com>
//
// Permission is hereby granted, free of charge, to any person obtaining a copy
// of this software and associated documentation files (the "Software"), to deal
// in the Software without restriction, including without limitation the rights
// to use, copy, modify, merge, publish, distribute, sublicense, and/or sell
// copies of the Software, and to permit persons to whom the Software is
// furnished to do so, subject to the following conditions:
//
// The above copyright notice and this permission notice shall be included in all
// copies or substantial portions of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS OR
// IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF MERCHANTABILITY,
// FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT SHALL THE
// AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM, DAMAGES OR OTHER
// LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR OTHERWISE, ARISING FROM,
// OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER DEALINGS IN THE
// SOFTWARE.


use std::fmt;


/// Decodes OpCode
pub const OP_CODE_UN_MASK: u8 = 0b0000_1111;

/// Encodes OpCode
pub const OP_CODE_MASK: u8 = 0b1000_0000;

/// Decodes payload
pub const PAYLOAD_KEY_UN_MASK: u8 = 0b0111_1111;

/// Continuation Op byte
pub const OP_CONTINUATION: u8 = 0x0;

/// Text Op byte
pub const OP_TEXT: u8 = 0x1;

/// Binary Op byte
pub const OP_BINARY: u8 = 0x2;

/// Close Op byte
pub const OP_CLOSE: u8 = 0x8;

/// Ping Op byte
pub const OP_PING: u8 = 0x9;

/// Pong Op byte
pub const OP_PONG: u8 = 0xA;


pub enum OpCode {
    /// Continuation frame from last packed
    Continuation,

    /// UTF-8 text data
    Text,

    /// Binary data as u8
    Binary,

    /// Indicates client has closed connection
    Close,

    /// Heartbeat requested from client
    Ping,

    /// Heartbeat response to ping frame
    /// Can also be sent without a ping request
    Pong
}

pub enum ReadError {
    /// fd is not a valid file descriptor or is not open for reading
    EBADF,

    /// buf is outside your accessible address space
    EFAULT,

    /// The call was interrupted by a signal before any data was read
    EINTR,

    /// fd is attached to an object which is unsuitable for reading;
    /// or the file was opened with the O_DIRECT flag, and either the address
    /// specified in buf, the value specified in count, or the current file
    /// offset is not suitably aligned
    EINVAL,

    /// I/O error. This will happen for example when the process is in a
    /// background process group, tries to read from its controlling tty,
    /// and either it is ignoring or blocking SIGTTIN or its process group
    /// is orphaned. It may also occur when there is a low-level I/O error
    /// while reading from a disk or tape.
    EIO,

    /// fd refers to a directory.
    EISDIR,

    /// Invalid OpCode.
    /// According to RFC-6455: "If an unknown opcode is received,
    /// the receiving endpoint MUST _Fail the WebSocket Connection_"
    OpCode,

    /// Returned when the number of bytes requested to read is greater than
    /// zero, but the number of bytes read was zero
    DataStop,

    /// Returned when there is no data waiting to be read
    NoData
}

pub enum WriteError {
    /// Non-blocking I/O has been selected using O_NONBLOCK and the
    /// write would block.
    EAGAIN,

    /// fd is not a valid file descriptor or is not open for writing.
    EBADF,

    /// buf is outside your accessible address space.
    EFAULT,

    /// An attempt was made to write a file that exceeds the
    /// implementation-defined maximum file size or the process’ file size
    /// limit, or to write at a position past the maximum allowed offset.
    EFBIG,

    /// The call was interrupted by a signal before any data was written.
    EINTR,

    /// fd is attached to an object which is unsuitable for writing; or the
    /// file was opened with the O_DIRECT flag, and either the address
    /// specified in buf, the value specified in count, or the current file
    /// offset is not suitably aligned.
    EINVAL,

    /// A low-level I/O error occurred while modifying the inode.
    EIO,

    /// The device containing the file referred to by fd has no room for
    /// the data.
    ENOSPC,

    /// fd is connected to a pipe or socket whose reading end is closed.
    /// When this happens the writing process will also receive a SIGPIPE
    /// signal. (Thus, the write return value is seen only if the program
    /// catches, blocks or ignores this signal.)
    EPIPE
}

pub enum SetFdError {
    /// Operation is prohibited by locks held by other processes.
    EACCES,

    /// Operation is prohibited by locks held by other processes.
    EAGAIN,

    /// fd is not an open file descriptor, or the command was F_SETLK or
    /// F_SETLKW and the file descriptor open mode doesn’t match with the
    /// type of lock requested.
    EBADF,

    /// It was detected that the specified F_SETLKW command would
    /// cause a deadlock.
    EDEADLK,

    /// lock is outside your accessible address space.
    EFAULT,

    /// For F_SETLKW, the command was interrupted by a signal. For F_GETLK
    /// and F_SETLK, the command was interrupted by a signal before the lock
    /// was checked or acquired. Most likely when locking a remote file
    /// (e.g. locking over NFS), but can sometimes happen locally.
    EINTR,

    /// For F_DUPFD, arg is negative or is greater than the maximum allowable
    /// value. For F_SETSIG, arg is not an allowable signal number.
    EINVAL,

    /// For F_DUPFD, the process already has the maximum number of file
    /// descriptors open.
    EMFILE,

    /// Too many segment locks open, lock table is full, or a remote locking
    /// protocol failed (e.g. locking over NFS).
    ENOLCK,

    /// Attempted to clear the O_APPEND flag on a file that has the
    /// append-only attribute set.
    EPERM
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            ReadError::EBADF    => "EBADF".fmt(f),
            ReadError::EFAULT   => "EFAULT".fmt(f),
            ReadError::EINTR    => "EINTR".fmt(f),
            ReadError::EINVAL   => "EINVAL".fmt(f),
            ReadError::EIO      => "EIO".fmt(f),
            ReadError::EISDIR   => "EISDIR".fmt(f),
            ReadError::OpCode   => "OpCode".fmt(f),
            ReadError::DataStop => "DataStop".fmt(f),
            ReadError::NoData   => "NoData".fmt(f)
        }
    }
}

impl fmt::Display for WriteError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            WriteError::EAGAIN  => "EAGAIN".fmt(f),
            WriteError::EBADF   => "EBADF".fmt(f),
            WriteError::EFAULT  => "EFAULT".fmt(f),
            WriteError::EFBIG   => "EFBIG".fmt(f),
            WriteError::EINTR   => "EINTR".fmt(f),
            WriteError::EINVAL  => "EINVAL".fmt(f),
            WriteError::EIO     => "EIO".fmt(f),
            WriteError::ENOSPC  => "ENOSPC".fmt(f),
            WriteError::EPIPE   => "EPIPE".fmt(f)
        }
    }
}

impl fmt::Display for SetFdError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            SetFdError::EACCES => "EACCES".fmt(f),
            SetFdError::EAGAIN => "EAGAIN".fmt(f),
            SetFdError::EBADF => "EBADF".fmt(f),
            SetFdError::EDEADLK => "EDEADLK".fmt(f),
            SetFdError::EFAULT => "EFAULT".fmt(f),
            SetFdError::EINTR => "EINTR".fmt(f),
            SetFdError::EINVAL => "EINVAL".fmt(f),
            SetFdError::EMFILE => "EMFILE".fmt(f),
            SetFdError::ENOLCK => "ENOLCK".fmt(f),
            SetFdError::EPERM => "EPERM".fmt(f)
        }
    }
}
