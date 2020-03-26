extern crate libc;
extern crate structopt;

use libc::c_void;
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::ptr;
use structopt::StructOpt;

#[derive(StructOpt, Debug)]
#[structopt(name = "diskbenchi")]
struct Opt {
    #[structopt(long)]
    bs: usize,

    #[structopt(long)]
    count: usize,

    #[structopt(long, parse(from_os_str))]
    of: PathBuf,

    #[structopt(long)]
    offset: Option<u64>,

    #[structopt(long)]
    hugepool: bool,
}

#[derive(Debug)]
pub struct HugePool {
    pool: *mut libc::c_void,
    len: usize,
}

impl Drop for HugePool {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.pool, self.len) };
    }
}

impl HugePool {
    pub fn to_slice(&self, len: usize) -> &[u8] {
        assert!(len <= self.len);
        unsafe { std::slice::from_raw_parts(self.pool as *const u8, len) }
    }

    pub fn to_mut_slice(&self, len: usize) -> &mut [u8] {
        assert!(len <= self.len);
        unsafe { std::slice::from_raw_parts_mut(self.pool as *mut u8, len) }
    }

    pub fn new(len: usize) -> Option<HugePool> {
        if len % 4096 != 0 {
            return None;
        }
        unsafe {
            let ptr = libc::mmap(
                ptr::null_mut(),
                len,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_HUGETLB | libc::MAP_ANONYMOUS,
                0,
                0,
            );

            if ptr == libc::MAP_FAILED {
                None
            } else if ptr as usize % 4096 == 0 {
                Some(HugePool { pool: ptr, len })
            } else {
                // failed
                libc::munmap(ptr, len);
                None
            }
        }
    }
}

#[derive(Debug)]
pub struct Pool {
    pool: *mut libc::c_void,
    len: usize,
}

impl Drop for Pool {
    fn drop(&mut self) {
        unsafe { libc::free(self.pool) };
    }
}

impl Pool {
    pub fn to_slice(&self, len: usize) -> &[u8] {
        assert!(len <= self.len);
        unsafe { std::slice::from_raw_parts(self.pool as *const u8, len) }
    }

    pub fn to_mut_slice(&self, len: usize) -> &mut [u8] {
        assert!(len <= self.len);
        unsafe { std::slice::from_raw_parts_mut(self.pool as *mut u8, len) }
    }

    pub fn new(len: usize) -> Option<Pool> {
        if len % 4096 != 0 {
            return None;
        }
        unsafe {
            let mut ptr: *mut c_void = ptr::null_mut();
            if libc::posix_memalign(&mut ptr, 4096, len) != 0 {
                // failed
                None
            } else {
                Some(Pool { pool: ptr, len })
            }
        }
    }
}

fn main() {
    let opt = Opt::from_args();
    let offset = opt.offset.unwrap_or(0);
    let hugepool = opt.hugepool;

    println!("{:#?}", opt);

    assert!(opt.bs % 4096 == 0);

    let mut file = OpenOptions::new()
        .create(true) // create if the designated file doesn't exist
        .write(true)
        .custom_flags(libc::O_DIRECT) // open with O_DIRECT
        .open(opt.of)
        .expect("Can't open");

    file.seek(SeekFrom::Start(offset)).expect("Can't seek");

    if !hugepool {
        let pool = Pool::new(opt.bs).expect("Can't make pool");
        for _ in 0..opt.count {
            file.write_all(pool.to_slice(opt.bs)).expect("Write failed");
        }
    } else {
        let huge_pool = HugePool::new(10 * 1024 * 1024)
            .expect("Can't make huge pool: check /proc/sys/vm/nr_hugepages");
        for _ in 0..opt.count {
            file.write_all(huge_pool.to_slice(opt.bs))
                .expect("Write failed");
        }
    }
}
