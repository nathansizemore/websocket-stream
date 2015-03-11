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


//! WebSocketStream crate

#![feature(libc, net, std_misc, os, collections, core)]

extern crate libc;

use std::mem;
use std::net::TcpStream;
use std::os;
use std::os::unix::prelude::AsRawFd;

use self::libc::consts::os::posix88;
use self::libc::{size_t, c_void, c_int, ssize_t};

use util::*;
pub mod util;

extern "system" {
    fn read(fd: c_int, buffer: *mut c_void, count: size_t) -> ssize_t;
    fn write(fd: c_int, buffer: *const c_void, cout: size_t) -> ssize_t;
}


pub type NewResult = Result<WebSocketStream, SetFdError>;
pub type ReadResult = Result<(OpCode, Vec<u8>), ReadError>;
pub type WriteResult = Result<u64, WriteError>;

type SysReadResult = Result<Vec<u8>, ReadError>;
type SysWriteResult = Result<u64, WriteError>;
type PayloadKeyResult = Result<u8, ReadError>;
type PayloadLenResult = Result<u64, ReadError>;

/// Represents a non-blocking RFC-6455 Protocol stream
pub struct WebSocketStream {
    stream: TcpStream
}

impl WebSocketStream {

    /// Creates a new WebSocket
    pub fn new(stream: TcpStream) -> NewResult {
        // Set fd as non-blocking
        let fd = stream.as_raw_fd();
        let mut response;
        unsafe {
            response = libc::fcntl(
                fd,
                libc::consts::os::posix01::F_SETFL,
                libc::consts::os::extra::O_NONBLOCK);
        }

        if response < 0 {
            let errno = os::errno();
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
                _ => panic!("Unknown errno from fcntl: {}", errno)
            };
        }

        Ok(WebSocketStream {
            stream: stream
        })
    }

    /// Attempts to read data from the socket.
    ///
    /// If data is available, it waits until a complete message has
    /// been received.
    /// It will return immediately if no data is available, or data is present,
    /// but a complete message is not yet available.  The previously read
    /// sections of that message will be discarded.  Non-blocking, ftw!
    pub fn read(&mut self) -> ReadResult {
        match self.check_for_data() {
            Ok(buf) => {
                if buf.len() > 0 {
                    // Ensure opcode is valid
                    let op_code = buf[0] & OP_CODE_UN_MASK;
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

                    // Get payload
                    match self.read_payload() {
                        Ok(payload) => Ok((op, payload)),
                        Err(e) => Err(e)
                    }
                } else {
                    Err(ReadError::NoData)
                }
            }
            Err(e) => Err(e)
        }
    }

    fn check_for_data(&mut self) -> SysReadResult {
        self.read_num_bytes(1)
    }

    fn read_payload(&mut self) -> SysReadResult {
        match self.read_payload_key() {
            Ok(key) => {
                match self.read_payload_len(key) {
                    Ok(len) => {
                        match self.read_masking_key() {
                            Ok(mask) => {
                                match self.read_num_bytes(len as usize) {
                                    Ok(buf) => {
                                        if buf.len() < 1 {
                                            return Err(ReadError::DataStop);
                                        }

                                        let mut payload = Vec::<u8>::
                                            with_capacity(buf.len());
                                        for x in 0..buf.len() {
                                            payload.push(buf[x] ^ mask[x % 4]);
                                        }
                                        Ok(payload)
                                    }
                                    Err(e) => Err(e)
                                }
                            }
                            Err(e) => Err(e)
                        }
                    }
                    Err(e) => Err(e)
                }
            }
            Err(e) => Err(e)
        }
    }

    fn read_payload_key(&mut self) -> PayloadKeyResult {
        match self.read_num_bytes(1) {
            Ok(key_buf) => {
                if key_buf.len() > 0 {
                    Ok(key_buf[0] & PAYLOAD_KEY_UN_MASK)
                } else {
                    Err(ReadError::DataStop)
                }
            }
            Err(e) => Err(e)
        }
    }

    fn read_payload_len(&mut self, key: u8) -> PayloadLenResult {
        match key {
            126 => {
                match self.read_num_bytes(2) {
                    Ok(buf) => {
                        let mut len = 0u16;
                        len = len | buf[0] as u16;
                        len = (len << 8) ^ buf[1] as u16;
                        Ok(len as u64)
                    }
                    Err(e) => Err(e)
                }
            }
            127 => {
                match self.read_num_bytes(8) {
                    Ok(buf) => {
                        let mut len = 0u64;
                        len = len | buf[0] as u64;
                        len = (len << 8) ^ buf[1] as u64;
                        len = (len << 8) ^ buf[2] as u64;
                        len = (len << 8) ^ buf[3] as u64;
                        len = (len << 8) ^ buf[4] as u64;
                        len = (len << 8) ^ buf[5] as u64;
                        len = (len << 8) ^ buf[6] as u64;
                        len = (len << 8) ^ buf[7] as u64;
                        Ok(len)
                    }
                    Err(e) => Err(e)
                }
            }
            _ => Ok(key as u64)
        }
    }

    fn read_masking_key(&mut self) -> SysReadResult {
        match self.read_num_bytes(4) {
            Ok(mask_buf) => {
                if mask_buf.len() > 0 {
                    Ok(mask_buf)
                } else {
                    Err(ReadError::DataStop)
                }
            }
            Err(e) => Err(e)
        }
    }

    fn read_num_bytes(&mut self, count: usize) -> SysReadResult {
        let mut temp_count = count;
        let mut total_read: usize = 0;
        let mut final_buffer: Vec<u8> = Vec::with_capacity(count);

        while total_read < count {
            let mut num_read;
            let mut buffer = [0u8; 512];
            let fd = self.stream.as_raw_fd();

            unsafe {
                let buf_ptr = buffer.as_mut_ptr();
                let void_buf_ptr: *mut c_void = mem::transmute(buf_ptr);
                num_read = read(fd, void_buf_ptr, temp_count as size_t);
            }

            if num_read > 0 {
                for x in 0..num_read {
                    final_buffer.push(buffer[x as usize]);
                }

                total_read += num_read as usize;
                temp_count -= num_read as usize;
            } else if num_read == 0 && total_read == 0 {
                return Ok(final_buffer);
            } else {
                let errno = os::errno();
                return match errno {
                    posix88::EBADF      => Err(ReadError::EBADF),
                    posix88::EFAULT     => Err(ReadError::EFAULT),
                    posix88::EINTR      => Err(ReadError::EINTR),
                    posix88::EINVAL     => Err(ReadError::EINVAL),
                    posix88::EIO        => Err(ReadError::EIO),
                    posix88::EISDIR     => Err(ReadError::EISDIR),
                    _ => Ok(Vec::<u8>::new()),
                };
            }
        }
        Ok(final_buffer)
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
        let buffer = buf.as_slice();
        let fd = self.stream.as_raw_fd();
        let count = buf.len() as size_t;

        let mut num_written;
        unsafe {
            let buff_ptr = buffer.as_ptr();
            let void_buff_ptr: *const c_void = mem::transmute(buff_ptr);
            num_written = write(fd, void_buff_ptr, count);
        }

        if num_written < 0 {
            let errno = os::errno();
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
                _ => panic!("Unknown errno: {}", errno),
            }
        }
        Ok(num_written as u64)
    }
}

impl Clone for WebSocketStream {
    fn clone(&self) -> WebSocketStream {
        WebSocketStream {
            stream: self.stream.try_clone().unwrap()
        }
    }
}
