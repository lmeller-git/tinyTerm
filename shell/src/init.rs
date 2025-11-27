use libtinyos::syscalls::{self, FileDescriptor, STDIN_FILENO};

pub fn init() -> FileDescriptor {
    // the first 4 bytes sent over stdin by term will be the fd holding the signal pipe.
    // TODO
    // if this is not spawned by term, we should bail. Not sure, how to enforce this
    // --> could send some pre-defined exchange signals over some stdout/stdin?
    // Ie. send GetSignalFD in stdout -> term receives this -> sends SendSignalFd Fd over Stdin
    // -> if we get something else we bail -> send back ShellErr or ShellSetUp to start process?
    let mut signal_fd_buf = [0_u8; 4];
    while unsafe { syscalls::read(STDIN_FILENO, signal_fd_buf.as_mut_ptr(), 4, 0) }.unwrap() == 0 {}

    let signal_fd = u32::from_be_bytes(signal_fd_buf);
    signal_fd
}
