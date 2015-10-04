websocket-stream [<img src="https://travis-ci.org/nathansizemore/websocket-stream.png?branch=master">](https://travis-ci.org/nathansizemore/websocket-stream)
================

websocket-stream is a [RFC-6455](https://tools.ietf.org/html/rfc6455)
wrapper for [TcpStream](http://doc.rust-lang.org/std/net/struct.TcpStream.html)
on POSIX-like kernels. It can be used in blocking or non-blocking mode.

I don't do enough Windows development to care about ensuring it is platform agnostic. Feel free to send me a pull request if you want to take the time to implement Windows sockets as well.

It achieves it's non-blocking state by setting the `O_NONBLOCK` flag on the
stream's file descriptor.

### Example Usage
~~~rust
extern crate websocket_stream as wss;

use wss::{WebsocketStream, ReadResult, WriteResult, Mode};
use wss::util::{OpCode, ReadError, WriteError};

fn some_function() {
    // stream is some std::net::TcpStream
    let mut ws_stream = match WebsocketStream::new(stream, Mode::NonBlock) {
        Ok(ws) => ws,
        Err(e) => {
            // This arm is hit when the system does not support 0_NONBLOCK
            panic!("Websocket creation failed, errno: {}", e)
        }
    };

    // Read a thing
    match ws_stream.read() {
        Ok(res_tuple) => {
            match res_tuple.0 {
                OpCode::Continuation    => handle_cont(res_tuple.1),
                OpCode::Text            => handle_text(res_tuple.1),
                OpCode::Binary          => handle_binary(res_tuple.1),
                OpCode::Close           => handle_close(res_tuple.1),
                OpCode::Ping            => handle_ping(res_tuple.1),
                OpCode::Pong            => handle_pong(res_tuple.1)
            }
        }
        Err(e) => {
            match e {
                ReadError::EAGAIN => {
                    // This arm is hit in Mode::NonBlock
                    // Signifies there was no data to read
                }
                _ => {
                    // This arm is hit on syscall level errors.
                    // ReadError can be printed for details
                }
            }
        }
    }

    // Write a thing
    let mut buf: Vec<u8> = Vec::new(); // Buffer full of awesome
    match ws_stream.write(OpCode::Text, &mut buf) {
        Ok(num_written) => {
            // Obv, num_written is the amount of bytes written
        }
        Err(e) => {
            // This arm is hit on syscall level errors.
            // WriteError can be printed for details
        }
    }
}
~~~
