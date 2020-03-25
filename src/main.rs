extern crate libc;
extern crate rand;
extern crate structopt;

use libc::c_void;
use rand::{thread_rng, Rng};
use std::fs::OpenOptions;
use std::io::{Seek, SeekFrom, Write};
use std::os::unix::fs::OpenOptionsExt;
use std::path::PathBuf;
use std::ptr;
use std::time::{Duration, Instant};
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
}

#[derive(Debug)]
pub struct HugePool {
    pool: *mut libc::c_void,
    aligned: *mut libc::c_void,
    len: usize,
}

unsafe impl Sync for HugePool {}
unsafe impl Send for HugePool {}

impl Drop for HugePool {
    fn drop(&mut self) {
        unsafe { libc::munmap(self.pool, self.len) };
    }
}

impl HugePool {
    pub fn to_slice(&self, len: usize) -> &[u8] {
        unsafe { std::slice::from_raw_parts(self.aligned as *const u8, len) }
    }

    pub fn to_mut_slice(&self, len: usize) -> &mut [u8] {
        unsafe { std::slice::from_raw_parts_mut(self.aligned as *mut u8, len) }
    }

    pub fn as_mutref(&self) -> *mut u8 {
        self.aligned as *mut u8
    }

    pub fn new(len: usize) -> Option<HugePool> {
        if len % 4096 != 0 {
            return None;
        }
        let len = len + 4096;

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
                dbg!("Good!");
                Some(HugePool {
                    pool: ptr,
                    aligned: ptr,
                    len,
                })
            } else {
                dbg!("So-so");
                Some(HugePool {
                    pool: ptr,
                    aligned: (4096 * ((ptr as usize + 4095) / 4096)) as *mut c_void,
                    len,
                })
            }
        }
    }
}

fn main() {
    let opt = Opt::from_args();
    let offset = opt.offset.unwrap_or(0);

    println!("{:#?}", opt);

    let mut file = OpenOptions::new()
        .create(true)
        .write(true)
        .custom_flags(libc::O_DIRECT)
        .open(opt.of)
        .expect("Can't open");

    file.seek(SeekFrom::Start(offset)).expect("Can't seek");
    
    let huge_pool = HugePool::new(10 * 1024 * 1024).expect("Can't make pool");
    
    let mut total_time1 = Duration::new(0, 0);
    let total_time2 = Instant::now();
    for _ in 0..opt.count {
        thread_rng().fill(huge_pool.to_mut_slice(opt.bs));

        let now = Instant::now();
        file.write_all(huge_pool.to_slice(opt.bs)).expect("Write failed");
        total_time1 += now.elapsed();
    }

    dbg!(total_time1);
    dbg!(total_time2.elapsed());
}
