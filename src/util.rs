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


/// Decodes OpCode in first byte received
pub const OP_CODE_MASK:     u8 = 0b0000_1111;

/// Decodes payload bytes
pub const PAYLOAD_KEY_MASK: u8 = 0b0111_1111;

pub const OP_CONTINUATION:  u8 = 0x0;
pub const OP_TEXT:          u8 = 0x1;
pub const OP_BINARY:        u8 = 0x2;
pub const OP_CLOSE:         u8 = 0x8;
pub const OP_PING:          u8 = 0x9;
pub const OP_PONG:          u8 = 0xA;


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
    EBADF,
    EFAULT,
    EINTR,
    EINVAL,
    EIO,
    EISDIR,
    OpCode,
    DataStop,
    NoData
}

pub enum WriteError {
    EBADF,
    EFAULT,
    EFBIG,
    EINTR,
    EINVAL,
    EIO,
    ENOSPC,
    EPIPE
}

pub enum SetFlError {
    EACCES,
    EAGAIN,
    EBADF,
    EDEADLK,
    EFAULT,
    EINTR,
    EINVAL,
    EMFILE,
    ENOLCK,
    EPERM
}
