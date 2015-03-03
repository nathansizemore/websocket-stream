websocket-stream
================

websocket-stream is a non-blocking [RFC-6455](https://tools.ietf.org/html/rfc6455)
wrapper for [TcpStream](http://doc.rust-lang.org/std/net/struct.TcpStream.html)
on POSIX-like kernels. I don't do enough Windows development to care about
implementing it for that configuration. Feel free to send me a pull
request if you want to take the time to implement Windows sockets as well.

It achieves it's non-blocking state by setting the `O_NONBLOCK` flag on the
stream's file descriptor. Aside from the system calls, the entire API is
memory safe.

### Why another Websocket Thing in Rust?

There are a lot of Websocket libraries out for Rust right now, but they all
force the same model of server design on the library user. Most implement
Websockets by creating a thread for each connection to read, and one to write.
Mainly to overcome Rust's default IO implementation as blocking by default.
This is wonderful, unless your server is expected to handle *lots* of
concurrent connections for long periods of time. Context switching between
200k threads will absolutely kill any gains you get by having separate read and
write operations.

### Example Usage

~~~rust
extern crate "websocket-stream" as wss;

use wss::WebSocketStream;
use wss::util::{OpCode, ReadError, WriteError};

fn some_function() {
    // stream is some std::net::TcpStream
    let mut ws_stream = match WebSocketStream::new(stream) {
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
                ReadError::NoData => {
                    // This arm is hit when there is nothing in the buffer
                }
                ReadError::DataStop => {
                    // This arm is hit when the buffer was exhausted before
                    // the message was read in full. The partial message is
                    // returned
                }
                _ => {
                    // This arm is hit on syscall level errors.
                    // Errno is set and can be printed for details
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
            // Errno is set and can be printed for details
        }
    }
}
~~~
