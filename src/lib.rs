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

#![feature(libc, net, std_misc, os)]

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


pub type NewResult = Result<WebSocketStream, SetFlError>;
pub type ReadResult = Result<(OpCode, Vec<u8>), ReadError>;
pub type WriteResult = Result<u64, WriteError>;

type SysReadResult = Result<Vec<u8>, ReadError>;
type SysWriteResult = Result<u64, WriteError>;
type PayloadKeyResult = Result<u8, ReadError>;
type PayloadLenResult = Result<u64, ReadError>;


pub struct WebSocketStream {
    stream: TcpStream
}

impl WebSocketStream {

    /// Creates a new non-blocking websocket
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
                posix88::EACCES     => Err(SetFlError::EACCES),
                posix88::EAGAIN     => Err(SetFlError::EAGAIN),
                posix88::EBADF      => Err(SetFlError::EBADF),
                posix88::EDEADLK    => Err(SetFlError::EDEADLK),
                posix88::EFAULT     => Err(SetFlError::EFAULT),
                posix88::EINTR      => Err(SetFlError::EINTR),
                posix88::EINVAL     => Err(SetFlError::EINVAL),
                posix88::EMFILE     => Err(SetFlError::EMFILE),
                posix88::ENOLCK     => Err(SetFlError::ENOLCK),
                posix88::EPERM      => Err(SetFlError::EPERM),
                _ => panic!("Unknown errno from fcntl: {}", errno)
            };
        }

        Ok(WebSocketStream {
            stream: stream
        })
    }

    pub fn read(&mut self) -> ReadResult {
        match self.check_for_data() {
            Ok(buf) => {
                if buf.len() > 0 {
                    // Ensure opcode is valid
                    let op_code = buf[0] & OP_CODE_MASK;
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
                    Ok(key_buf[0] & PAYLOAD_KEY_MASK)
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
            } else if num_read == 0 {
                return Ok(Vec::<u8>::new());
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
}
