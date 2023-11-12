use core::arch::x86_64::*;
use std::hint::black_box;

#[inline(always)]
fn bench() {
    unsafe { syscalls::raw_syscall!(syscalls::Sysno::getpid) };
}

#[inline(always)]
fn serialized_time() -> u64 {
    unsafe {
        _mm_lfence();
        _mm_mfence();
        _mm_sfence();
        __cpuid(0);
        _mm_lfence();
        _mm_mfence();
        _mm_sfence();
        let result = _rdtsc();
        _mm_lfence();
        _mm_mfence();
        _mm_sfence();
        __cpuid(0);
        _mm_lfence();
        _mm_mfence();
        _mm_sfence();
        result
    }
}

const NUM_RUNS: u64 = 100_000;

fn main() {
    assert!(core_affinity::set_for_current(core_affinity::CoreId {
        id: 0
    }));

    let mut total_difference: u64 = 0;
    for _ in 0..NUM_RUNS {
        let start = serialized_time();
        bench();
        let end = serialized_time();
        total_difference += end - start;
    }

    println!("elapsed cycles: {}", total_difference);
    println!("average cycles: {}", total_difference / NUM_RUNS);
}
