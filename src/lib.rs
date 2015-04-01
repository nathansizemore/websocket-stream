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


//! WebsocketStream crate

 #![feature(io_ext, collections)]

extern crate libc;
extern crate errno;

use std::{mem, ptr, fmt};
use std::result::Result;
use std::net::TcpStream;
use std::os::unix::io::AsRawFd;

use self::errno::errno;
use self::libc::consts::os::posix88;
use self::libc::{size_t, c_void, c_int, ssize_t};

use util::*;
pub mod util;


extern "system" {
    fn read(fd: c_int, buffer: *mut c_void, count: size_t) -> ssize_t;
    fn write(fd: c_int, buffer: *const c_void, cout: size_t) -> ssize_t;
}

/// Represents the result of trying to create a WebsocketStream
pub type NewResult = Result<WebsocketStream, SetFdError>;

/// Represents the result of setting a flag to file descriptor through syscalls
pub type SetFdResult = Result<(), SetFdError>;

/// Represents the result of attempting a syscall read on the file descriptor
pub type ReadResult = Result<(OpCode, Vec<u8>), ReadError>;

/// Represents the result of attempting a syscall write on the file descriptor
pub type WriteResult = Result<u64, WriteError>;

/// Internal result of read syscall on file descriptor
type SysReadResult = Result<(), ReadError>;

/// Internal result of write syscall on file descriptor
type SysWriteResult = Result<u64, WriteError>;

/// Internal result of attempting to retrieve the OpCode
/// https://tools.ietf.org/html/rfc6455#page-29
type OpCodeResult = Result<OpCode, ReadError>;

/// Internal result of attempting to retrieve the payload key
/// https://tools.ietf.org/html/rfc6455#page-29
type PayloadKeyResult = Result<u8, ReadError>;

/// Internal result of attempting to read the payload length
/// https://tools.ietf.org/html/rfc6455#page-29
type PayloadLenResult = Result<u64, ReadError>;


/// RFC-6455 Protocol stream
pub struct WebsocketStream {
    mode: Mode,
    stream: TcpStream,
    state: State,
    msg: Message,
    buffer: Buffer
}

/// Generic buffer used for reading/writing
#[derive(Clone)]
struct Buffer {
    remaining: usize,
    buf: Vec<u8>
}

/// Websocket Frame
#[derive(Clone)]
struct Message {
    op_code: OpCode,
    payload_key: u8,
    payload_len: u64,
    masking_key: [u8; 4],
    payload: Vec<u8>
}

/// Stream I/O mode
#[derive(PartialEq, Clone)]
pub enum Mode {
    /// Blocking I/O
    Block,
    /// Non-blocking I/O
    NonBlock
}

/// Stream state
#[derive(PartialEq, Clone)]
pub enum State {
    OpCode,
    PayloadKey,
    PayloadLength,
    MaskingKey,
    Payload
}

impl WebsocketStream {

    /// Attempts to create a new stream in specified mode
    pub fn new(stream: TcpStream, mode: Mode) -> NewResult {
        match mode {
            Mode::Block => {
                Ok(WebsocketStream {
                    stream: stream,
                    mode: Mode::Block,
                    state: State::OpCode,
                    msg: Message {
                        op_code: OpCode::Text,
                        payload_key: 0u8,
                        payload_len: 0u64,
                        masking_key: [0u8; 4],
                        payload: Vec::new()
                    },
                    buffer: Buffer {
                        remaining: 1,
                        buf: Vec::new()
                    }
                })
            }
            Mode::NonBlock => {
                match WebsocketStream::set_non_block(&stream) {
                    Ok(()) => Ok(WebsocketStream {
                        stream: stream,
                        mode: Mode::NonBlock,
                        state: State::OpCode,
                        msg: Message {
                            op_code: OpCode::Text,
                            payload_key: 0u8,
                            payload_len: 0u64,
                            masking_key: [0u8; 4],
                            payload: Vec::new()
                        },
                        buffer: Buffer {
                            remaining: 1,
                            buf: Vec::new()
                        }
                    }),
                    Err(e) => Err(e)
                }
            }
        }
    }

    /// Sets the socket to the specified mode
    pub fn set_mode(&mut self, mode: Mode) -> SetFdResult {
        // If we're already in the mode, no need to for a syscall
        if self.mode == mode {
            return Ok(())
        }

        match mode {
            Mode::Block => {
                WebsocketStream::set_block(&self.stream)
            }
            Mode::NonBlock => {
                WebsocketStream::set_non_block(&self.stream)
            }
        }
    }

    fn set_block(stream: &TcpStream) -> SetFdResult {
        let fd = stream.as_raw_fd();

        // Get the flags currently set on the fd
        let mut flags;
        unsafe {
            flags = libc::fcntl(fd, libc::consts::os::posix01::F_GETFL);
        }

        // Ensure we were able to get the current set flags
        if flags < 0 {
            let errno = errno().0 as i32;
            return match errno {
                posix88::EACCES     => Err(SetFdError::EACCES),
                posix88::EAGAIN     => Err(SetFdError::EAGAIN),
                posix88::EBADF      => Err(SetFdError::EBADF),
                posix88::EDEADLK    => Err(SetFdError::EDEADLK),
                posix88::EFAULT     => Err(SetFdError::EFAULT),
                posix88::EINTR      => Err(SetFdError::EINTR),
                posix88::EINVAL     => Err(SetFdError::EINVAL),
                posix88::EMFILE     => Err(SetFdError::EMFILE),
                posix88::ENOLCK     => Err(SetFdError::ENOLCK),
                posix88::EPERM      => Err(SetFdError::EPERM),
                _ => panic!("Unexpected errno: {}", errno)
            };
        }

        // Remove non-blocking flag
        let mut response;
        unsafe {
            response = libc::fcntl(
                fd,
                libc::consts::os::posix01::F_SETFL,
                flags & !libc::consts::os::extra::O_NONBLOCK);
        }

        // Ensure removal was successful
        if response < 0 {
            let errno = errno().0 as i32;
            return match errno {
                posix88::EACCES     => Err(SetFdError::EACCES),
                posix88::EAGAIN     => Err(SetFdError::EAGAIN),
                posix88::EBADF      => Err(SetFdError::EBADF),
                posix88::EDEADLK    => Err(SetFdError::EDEADLK),
                posix88::EFAULT     => Err(SetFdError::EFAULT),
                posix88::EINTR      => Err(SetFdError::EINTR),
                posix88::EINVAL     => Err(SetFdError::EINVAL),
                posix88::EMFILE     => Err(SetFdError::EMFILE),
                posix88::ENOLCK     => Err(SetFdError::ENOLCK),
                posix88::EPERM      => Err(SetFdError::EPERM),
                _ => panic!("Unexpected errno: {}", errno)
            };
        } else {
            Ok(())
        }
    }

    fn set_non_block(stream: &TcpStream) -> SetFdResult {
        let fd = stream.as_raw_fd();
        let mut response;
        unsafe {
            response = libc::fcntl(
                fd,
                libc::consts::os::posix01::F_SETFL,
                libc::consts::os::extra::O_NONBLOCK);
        }

        if response < 0 {
            let errno = errno().0 as i32;
            return match errno {
                posix88::EACCES     => Err(SetFdError::EACCES),
                posix88::EAGAIN     => Err(SetFdError::EAGAIN),
                posix88::EBADF      => Err(SetFdError::EBADF),
                posix88::EDEADLK    => Err(SetFdError::EDEADLK),
                posix88::EFAULT     => Err(SetFdError::EFAULT),
                posix88::EINTR      => Err(SetFdError::EINTR),
                posix88::EINVAL     => Err(SetFdError::EINVAL),
                posix88::EMFILE     => Err(SetFdError::EMFILE),
                posix88::ENOLCK     => Err(SetFdError::ENOLCK),
                posix88::EPERM      => Err(SetFdError::EPERM),
                _ => panic!("Unexpected errno: {}", errno)
            };
        } else {
            Ok(())
        }
    }

    /// Attempts to read data from the socket.
    ///
    /// If stream is in Mode::Block, this will block forever until
    /// data is received
    ///
    /// If socket is in Mode::NonBlock and data is available,
    /// it will read until a complete message is received.  If the buffer
    /// has run out, and it is still waiting on the remaining payload, it
    /// will attempt 3 more reads and then give up, disregarding the message.
    pub fn read(&mut self) -> ReadResult {

        // Read the OpCode
        if self.state == State::OpCode {
            if self.buffer.remaining == 0 {
                self.buffer.remaining = 1;
                self.buffer.buf = Vec::<u8>::with_capacity(1);
            }

            let result = self.read_op_code();
            if !result.is_ok() {
                return Err(result.unwrap_err());
            }
            self.msg.op_code = result.unwrap();

            // Set state to next stage
            self.state = State::PayloadKey;
            self.buffer.remaining = 1;
            self.buffer.buf = Vec::<u8>::with_capacity(1);
        }

        // Read the Payload Key
        if self.state == State::PayloadKey {
            let result = self.read_payload_key();
            if !result.is_ok() {
                return Err(result.unwrap_err());
            }
            self.msg.payload_key = result.unwrap();

            // Set next state
            self.state = State::PayloadLength;
            self.buffer.remaining = match self.msg.payload_key {
                127 => 8,
                126 => 2,
                _ => {
                    self.msg.payload_len = self.msg.payload_key as u64;
                    0
                }
            };
            self.buffer.buf = Vec::<u8>::with_capacity(self.buffer.remaining);
        }

        // Read the payload length, if needed
        if self.state == State::PayloadLength {
            if self.buffer.remaining > 0 {
                let result = self.read_payload_length();
                if !result.is_ok() {
                    let err = result.unwrap_err();
                    match err {
                        ReadError::EAGAIN => {
                            // Update bytes remaining
                            self.buffer.remaining = (self.msg.payload_len -
                                self.buffer.buf.len() as u64) as usize;
                        }
                        _ => { }
                    }
                    return Err(err);
                }

                // Grab result
                self.msg.payload_len = result.unwrap();

                // Update bytes remaining
                let bytes_needed = match self.msg.payload_key {
                    127 => 8,
                    126 => 2,
                    _ => 0
                };
                self.buffer.remaining = (bytes_needed -
                    self.buffer.buf.len() as u64) as usize;
            } else {
                // If buffer.remaining == 0, len was the key
                self.state = State::MaskingKey;
                self.buffer.remaining = 4;
                self.buffer.buf = Vec::<u8>::with_capacity(4);
            }
        }

        // Read the masking key
        if self.state == State::MaskingKey {
            let result = self.read_masking_key();
            if !result.is_ok() {
                let err = result.unwrap_err();
                match err {
                    ReadError::EAGAIN => {
                        // Update bytes remaining
                        self.buffer.remaining = 4 - self.buffer.buf.len();
                    }
                    _ => { }
                }
                return Err(err);
            }

            // Copy the masking key
            self.msg.masking_key[0] = self.buffer.buf[0];
            self.msg.masking_key[1] = self.buffer.buf[1];
            self.msg.masking_key[2] = self.buffer.buf[2];
            self.msg.masking_key[3] = self.buffer.buf[3];

            // Change state and update buffer
            self.state = State::Payload;
            self.buffer.remaining = self.msg.payload_len as usize;
            self.buffer.buf = Vec::<u8>::with_capacity(
                self.msg.payload_len as usize);
        }

        // Read the payload
        if self.state == State::Payload {
            let result = self.read_payload();
            if !result.is_ok() {
                let err = result.unwrap_err();
                match err {
                    ReadError::EAGAIN => {
                        // Update bytes remaining
                        self.buffer.remaining = (self.msg.payload_len -
                            self.buffer.buf.len() as u64) as usize;
                    }
                    _ => { }
                }
                return Err(err);
            }

            // Unmask the payload
            self.msg.payload = Vec::<u8>::with_capacity(
                self.msg.payload_len as usize);
            for x in 0..self.buffer.buf.len() {
                self.msg.payload.push(
                    self.buffer.buf[x] ^ self.msg.masking_key[x % 4]);
            }

            self.state = State::OpCode;
            self.buffer.remaining = 1;
            self.buffer.buf = Vec::<u8>::with_capacity(1);

            // Return the OpCode and Payload
            return Ok((self.msg.op_code.clone(), self.msg.payload.clone()))
        }

        // Default return value
        Err(ReadError::EAGAIN)
    }

    /// Attempts to read and unmask the OpCode frame
    fn read_op_code(&mut self) -> OpCodeResult {
        match self.read_num_bytes(1) {
            Ok(()) => { }
            Err(e) => return Err(e)
        };

        // Ensure opcode is valid
        let op_code = self.buffer.buf[0] & OP_CODE_UN_MASK;
        let valid_op = match op_code {
            OP_CONTINUATION => true,
            OP_TEXT         => true,
            OP_BINARY       => true,
            OP_CLOSE        => true,
            OP_PING         => true,
            OP_PONG         => true,
            _ => false
        };
        if !valid_op {
            return Err(ReadError::OpCode);
        }

        // Assign OpCode
        let op = match op_code {
            OP_CONTINUATION => OpCode::Continuation,
            OP_TEXT         => OpCode::Text,
            OP_BINARY       => OpCode::Binary,
            OP_CLOSE        => OpCode::Close,
            OP_PING         => OpCode::Ping,
            OP_PONG         => OpCode::Pong,
            _ => unimplemented!()
        };
        Ok(op)
    }

    /// Attempts to read the payload key from the socket
    fn read_payload_key(&mut self) -> PayloadKeyResult {
        match self.read_num_bytes(1) {
            Ok(()) => Ok(self.buffer.buf[0] & PAYLOAD_KEY_UN_MASK),
            Err(e) => Err(e)
        }
    }

    /// Attempts to read the payload length from the socket
    fn read_payload_length(&mut self) -> PayloadLenResult {
        let count = self.buffer.remaining;
        match self.read_num_bytes(count) {
            Ok(()) => {
                if self.msg.payload_key == 126 {
                    let mut len = 0u16;
                    len = len | self.buffer.buf[0] as u16;
                    len = (len << 8) | self.buffer.buf[1] as u16;
                    Ok(len as u64)
                } else {
                    let mut len = 0u64;
                    len = len | self.buffer.buf[0] as u64;
                    len = (len << 8) | self.buffer.buf[1] as u64;
                    len = (len << 8) | self.buffer.buf[2] as u64;
                    len = (len << 8) | self.buffer.buf[3] as u64;
                    len = (len << 8) | self.buffer.buf[4] as u64;
                    len = (len << 8) | self.buffer.buf[5] as u64;
                    len = (len << 8) | self.buffer.buf[6] as u64;
                    len = (len << 8) | self.buffer.buf[7] as u64;
                    Ok(len)
                }
            }
            Err(e) => Err(e)
        }
    }

    /// Attempts to read the masking key from the stream
    fn read_masking_key(&mut self) -> SysReadResult {
        let count = self.buffer.remaining;
        match self.read_num_bytes(count) {
            Ok(()) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Attempts to read the payload from the stream
    fn read_payload(&mut self) -> SysReadResult {
        let count = self.buffer.remaining;
        match self.read_num_bytes(count) {
            Ok(()) => Ok(()),
            Err(e) => Err(e)
        }
    }

    /// Attempts to read count bytes from the stream
    fn read_num_bytes(&mut self, count: usize) -> SysReadResult {
        let fd = self.stream.as_raw_fd();

        // Create a buffer for the total of bytes still needed
        let mut buffer;
        unsafe {
            buffer = libc::calloc(count as size_t,
                mem::size_of::<u8>() as size_t);
        }

        // Ensure system gave up the mem
        if buffer.is_null() {
            return Err(ReadError::ENOMEM)
        }

        // Read data into buffer
        let mut num_read;
        unsafe {
            num_read = read(fd, buffer, count as size_t);
        }

        // Report and exit on any thrown errors
        if num_read < 0 {
            unsafe { libc::free(buffer); }
            let errno = errno().0 as i32;
            return match errno {
                posix88::EBADF      => Err(ReadError::EBADF),
                posix88::EFAULT     => Err(ReadError::EFAULT),
                posix88::EINTR      => Err(ReadError::EINTR),
                posix88::EINVAL     => Err(ReadError::EINVAL),
                posix88::EIO        => Err(ReadError::EIO),
                posix88::EISDIR     => Err(ReadError::EISDIR),
                posix88::EAGAIN     => Err(ReadError::EAGAIN),
                _ => panic!("Unexpected errno during read: {}", errno)
            };
        }

        // Check for EOF
        if num_read == 0 {
            unsafe { libc::free(buffer); }
            return Err(ReadError::EAGAIN);
        }

        // Add bytes to msg buffer
        for x in 0..num_read as isize {
            unsafe {
                self.buffer.buf.push(ptr::read(buffer.offset(x)) as u8);
            }
        }

        // Free buffer and return Ok
        unsafe { libc::free(buffer); }
        Ok(())
    }

    /// Attempts to write data to the socket
    pub fn write(&mut self, op: OpCode, payload: &mut Vec<u8>) -> WriteResult {
        let mut out_buf: Vec<u8> = Vec::with_capacity(payload.len() + 9);

        self.set_op_code(&op, &mut out_buf);
        self.set_payload_info(payload.len(), &mut out_buf);
        out_buf.append(payload);

        self.write_bytes(&out_buf)
    }

    fn set_op_code(&self, op: &OpCode, buf: &mut Vec<u8>) {
        let op_code = match *op {
            OpCode::Continuation    => OP_CONTINUATION,
            OpCode::Text            => OP_TEXT,
            OpCode::Binary          => OP_BINARY,
            OpCode::Close           => OP_CLOSE,
            OpCode::Ping            => OP_PING,
            OpCode::Pong            => OP_PONG
        };
        buf.push(op_code | OP_CODE_MASK);
    }

    fn set_payload_info(&self, len: usize, buf: &mut Vec<u8>) {
        if len <= 125 {
            buf.push(len as u8);
        } else if len <= 65535 {
            let mut len_buf = [0u8; 2];
            len_buf[0] = (len as u16 & 0b1111_1111u16 << 8) as u8;
            len_buf[1] = (len as u16 & 0b1111_1111 as u16) as u8;

            buf.push(126u8); // 16 bit prelude
            buf.push(len_buf[0]);
            buf.push(len_buf[1]);
        } else {
            let mut len_buf = [0u8; 8];
            len_buf[0] = (len as u64 & 0b1111_1111u64 << 56) as u8;
            len_buf[1] = (len as u64 & 0b1111_1111u64 << 48) as u8;
            len_buf[2] = (len as u64 & 0b1111_1111u64 << 40) as u8;
            len_buf[3] = (len as u64 & 0b1111_1111u64 << 32) as u8;
            len_buf[4] = (len as u64 & 0b1111_1111u64 << 24) as u8;
            len_buf[5] = (len as u64 & 0b1111_1111u64 << 16) as u8;
            len_buf[6] = (len as u64 & 0b1111_1111u64 << 8) as u8;
            len_buf[7] = (len as u64 & 0b1111_1111u64) as u8;

            buf.push(127u8); // 64 bit prelude
            buf.push(len_buf[0]);
            buf.push(len_buf[1]);
            buf.push(len_buf[2]);
            buf.push(len_buf[3]);
            buf.push(len_buf[4]);
            buf.push(len_buf[5]);
            buf.push(len_buf[6]);
            buf.push(len_buf[7]);
        }
    }

    fn write_bytes(&mut self, buf: &Vec<u8>) -> SysWriteResult {
        let buffer = &buf[..];
        let fd = self.stream.as_raw_fd();
        let count = buf.len() as size_t;

        let mut num_written;
        unsafe {
            let buff_ptr = buffer.as_ptr();
            let void_buff_ptr: *const c_void = mem::transmute(buff_ptr);
            num_written = write(fd, void_buff_ptr, count);
        }

        if num_written < 0 {
            let errno = errno().0 as i32;
            return match errno {
                posix88::EAGAIN     => Err(WriteError::EAGAIN),
                posix88::EBADF      => Err(WriteError::EBADF),
                posix88::EFAULT     => Err(WriteError::EFAULT),
                posix88::EFBIG      => Err(WriteError::EFBIG),
                posix88::EINTR      => Err(WriteError::EINTR),
                posix88::EINVAL     => Err(WriteError::EINVAL),
                posix88::EIO        => Err(WriteError::EIO),
                posix88::ENOSPC     => Err(WriteError::ENOSPC),
                posix88::EPIPE      => Err(WriteError::EPIPE),
                _ => panic!("Unknown errno during write: {}", errno),
            }
        }
        Ok(num_written as u64)
    }
}

impl Clone for WebsocketStream {
    fn clone(&self) -> WebsocketStream {
        WebsocketStream {
            mode: self.mode.clone(),
            stream: self.stream.try_clone().unwrap(),
            state: self.state.clone(),
            msg: self.msg.clone(),
            buffer: self.buffer.clone()
        }
    }
}

impl fmt::Display for State {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match *self {
            State::OpCode => "OpCode".fmt(f),
            State::PayloadKey => "PayloadKey".fmt(f),
            State::PayloadLength => "PayloadLength".fmt(f),
            State::MaskingKey => "MaskingKey".fmt(f),
            State::Payload => "Payload".fmt(f)
        }
    }
}
