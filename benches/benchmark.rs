use std::{
    arch::{
        asm,
        x86_64::{__cpuid, _mm_lfence, _mm_mfence, _mm_sfence},
    },
    thread::yield_now,
    time::{Duration, Instant},
};

use criterion::{black_box, criterion_group, criterion_main, Criterion};
use futures::future::join_all;
use lazy_static::lazy_static;
use tokio::runtime::{Builder, Runtime};

fn multi_thread_tokio_runtime() -> Runtime {
    Builder::new_multi_thread().enable_all().build().unwrap()
}

fn fibonacci(n: u64) -> u64 {
    match black_box(n) {
        0 => black_box(1),
        1 => black_box(1),
        n => black_box(fibonacci(black_box(n - 1))) + black_box(fibonacci(black_box(n - 2))),
    }
}

const FIB_N: u64 = 30;
const SLEEP_MS: u64 = 25;

lazy_static! {
    static ref NUM_THREADS_SMALL: usize = core_affinity::get_core_ids().unwrap().len() / 2;
    static ref NUM_THREADS_LARGE: usize = *NUM_THREADS_SMALL * 8;
    static ref NUM_THREADS_HUGE: usize = *NUM_THREADS_SMALL * 256;
}

fn fib_benchmark(c: &mut Criterion) {
    c.bench_function("fibonacci(FIB_N)", |b| {
        b.iter(|| fibonacci(FIB_N));
    });
}

fn system_benchmark(c: &mut Criterion) {
    c.bench_function("spawn os thread", |b| {
        b.iter(|| std::thread::spawn(|| {}).join().unwrap())
    });

    c.bench_function("spawn single os thread expensive calculation", |b| {
        b.iter(|| std::thread::spawn(|| fibonacci(FIB_N)).join().unwrap())
    });

    c.bench_function("spawn single os thread sleep", |b| {
        b.iter(|| {
            std::thread::spawn(|| std::thread::sleep(Duration::from_millis(SLEEP_MS)))
                .join()
                .unwrap()
        })
    });

    c.bench_function(
        "spawn small multiple os thread expensive calculation",
        |b| {
            b.iter(|| {
                let threads = (0..*NUM_THREADS_SMALL)
                    .map(|_| std::thread::spawn(|| fibonacci(FIB_N)))
                    .collect::<Vec<_>>();

                threads.into_iter().map(|t| t.join().unwrap()).sum::<u64>();
            })
        },
    );

    c.bench_function("spawn small multiple os thread sleep", |b| {
        b.iter(|| {
            let threads = (0..*NUM_THREADS_SMALL)
                .map(|_| std::thread::spawn(|| std::thread::sleep(Duration::from_millis(SLEEP_MS))))
                .collect::<Vec<_>>();

            threads
                .into_iter()
                .map(|t| t.join().unwrap())
                .for_each(|_| {});
        })
    });

    c.bench_function(
        "spawn large multiple os thread expensive calculation",
        |b| {
            b.iter(|| {
                let threads = (0..*NUM_THREADS_LARGE)
                    .map(|_| std::thread::spawn(|| fibonacci(FIB_N)))
                    .collect::<Vec<_>>();

                threads.into_iter().map(|t| t.join().unwrap()).sum::<u64>();
            })
        },
    );

    c.bench_function("spawn large multiple os thread sleep", |b| {
        b.iter(|| {
            let threads = (0..*NUM_THREADS_LARGE)
                .map(|_| std::thread::spawn(|| std::thread::sleep(Duration::from_millis(SLEEP_MS))))
                .collect::<Vec<_>>();

            threads
                .into_iter()
                .map(|t| t.join().unwrap())
                .for_each(|_| {});
        })
    });

    c.bench_function("spawn huge multiple os thread sleep", |b| {
        b.iter(|| {
            let threads = (0..*NUM_THREADS_HUGE)
                .map(|_| std::thread::spawn(|| std::thread::sleep(Duration::from_millis(SLEEP_MS))))
                .collect::<Vec<_>>();

            threads
                .into_iter()
                .map(|t| t.join().unwrap())
                .for_each(|_| {});
        })
    });

    c.bench_function(
        "spawn os thread large worker huge sleep complex workload",
        |b| {
            b.iter(|| {
                let work_threads = (0..*NUM_THREADS_LARGE)
                    .map(|_| {
                        std::thread::spawn(|| {
                            fibonacci(FIB_N);
                        })
                    })
                    .collect::<Vec<_>>();

                let sleep_threads = (0..*NUM_THREADS_HUGE)
                    .map(|_| {
                        std::thread::spawn(|| std::thread::sleep(Duration::from_millis(SLEEP_MS)))
                    })
                    .collect::<Vec<_>>();

                work_threads
                    .into_iter()
                    .chain(sleep_threads)
                    .map(|t| t.join().unwrap())
                    .for_each(|_| {});
            })
        },
    );
}

fn tokio_benchmark(c: &mut Criterion) {
    c.bench_function("spawn tokio thread", |b| {
        b.to_async(multi_thread_tokio_runtime())
            .iter(|| async { tokio::task::spawn(async {}).await.unwrap() });
    });

    c.bench_function("spawn single tokio thread expensive calculation", |b| {
        b.to_async(multi_thread_tokio_runtime()).iter(|| async {
            tokio::task::spawn(async { fibonacci(FIB_N) })
                .await
                .unwrap()
        });
    });

    c.bench_function("spawn single tokio thread sleep", |b| {
        b.to_async(multi_thread_tokio_runtime()).iter(|| async {
            tokio::task::spawn(async { tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await })
                .await
                .unwrap()
        });
    });

    c.bench_function("spawn small tokio thread expensive calculation", |b| {
        b.to_async(multi_thread_tokio_runtime()).iter(|| async {
            let tasks = (0..*NUM_THREADS_SMALL)
                .map(|_| tokio::task::spawn(async { fibonacci(FIB_N) }))
                .collect::<Vec<_>>();

            join_all(tasks)
                .await
                .into_iter()
                .map(|res| res.unwrap())
                .sum::<u64>()
        });
    });

    c.bench_function("spawn small tokio thread sleep", |b| {
        b.to_async(multi_thread_tokio_runtime()).iter(|| async {
            let tasks = (0..*NUM_THREADS_SMALL)
                .map(|_| {
                    tokio::task::spawn(async {
                        tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await
                    })
                })
                .collect::<Vec<_>>();

            join_all(tasks)
                .await
                .into_iter()
                .map(|res| res.unwrap())
                .for_each(|_| {});
        });
    });

    c.bench_function("spawn large tokio thread expensive calculation", |b| {
        b.to_async(multi_thread_tokio_runtime()).iter(|| async {
            let tasks = (0..*NUM_THREADS_LARGE)
                .map(|_| tokio::task::spawn(async { fibonacci(FIB_N) }))
                .collect::<Vec<_>>();

            join_all(tasks)
                .await
                .into_iter()
                .map(|res| res.unwrap())
                .sum::<u64>()
        });
    });

    c.bench_function("spawn large tokio thread sleep", |b| {
        b.to_async(multi_thread_tokio_runtime()).iter(|| async {
            let tasks = (0..*NUM_THREADS_LARGE)
                .map(|_| {
                    tokio::task::spawn(async {
                        tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await
                    })
                })
                .collect::<Vec<_>>();

            join_all(tasks)
                .await
                .into_iter()
                .map(|res| res.unwrap())
                .for_each(|_| {});
        });
    });

    c.bench_function("spawn huge tokio thread sleep", |b| {
        b.to_async(multi_thread_tokio_runtime()).iter(|| async {
            let tasks = (0..*NUM_THREADS_HUGE)
                .map(|_| {
                    tokio::task::spawn(async {
                        tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await
                    })
                })
                .collect::<Vec<_>>();

            join_all(tasks)
                .await
                .into_iter()
                .map(|res| res.unwrap())
                .for_each(|_| {});
        });
    });

    c.bench_function(
        "spawn tokio thread large worker huge sleep complex workload worker tasks first",
        |b| {
            b.to_async(multi_thread_tokio_runtime()).iter(|| async {
                let work_tasks = (0..*NUM_THREADS_LARGE)
                    .map(|_| {
                        tokio::task::spawn(async {
                            fibonacci(FIB_N);
                        })
                    })
                    .collect::<Vec<_>>();

                let sleep_tasks = (0..*NUM_THREADS_HUGE)
                    .map(|_| {
                        tokio::task::spawn(async {
                            tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await
                        })
                    })
                    .collect::<Vec<_>>();

                join_all(work_tasks.into_iter().chain(sleep_tasks))
                    .await
                    .into_iter()
                    .map(|res| res.unwrap())
                    .for_each(|_| {});
            })
        },
    );

    c.bench_function(
        "spawn tokio thread large worker huge sleep complex workload sleep tasks first",
        |b| {
            b.to_async(multi_thread_tokio_runtime()).iter(|| async {
                let sleep_tasks = (0..*NUM_THREADS_HUGE)
                    .map(|_| {
                        tokio::task::spawn(async {
                            tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await
                        })
                    })
                    .collect::<Vec<_>>();

                let work_tasks = (0..*NUM_THREADS_LARGE)
                    .map(|_| {
                        tokio::task::spawn(async {
                            fibonacci(FIB_N);
                        })
                    })
                    .collect::<Vec<_>>();

                join_all(sleep_tasks.into_iter().chain(work_tasks))
                    .await
                    .into_iter()
                    .map(|res| res.unwrap())
                    .for_each(|_| {});
            })
        },
    );

    c.bench_function("spawn tokio blocking thread", |b| {
        b.to_async(multi_thread_tokio_runtime())
            .iter(|| async { tokio::task::spawn_blocking(|| {}).await.unwrap() });
    });

    c.bench_function(
        "spawn single tokio blocking thread expensive calculation",
        |b| {
            b.to_async(multi_thread_tokio_runtime()).iter(|| async {
                tokio::task::spawn_blocking(|| fibonacci(FIB_N))
                    .await
                    .unwrap()
            });
        },
    );

    c.bench_function("spawn single tokio blocking thread sleep", |b| {
        b.to_async(multi_thread_tokio_runtime()).iter(|| async {
            tokio::task::spawn_blocking(|| std::thread::sleep(Duration::from_millis(SLEEP_MS)))
                .await
                .unwrap()
        });
    });

    c.bench_function(
        "spawn small tokio blocking thread expensive calculation",
        |b| {
            b.to_async(multi_thread_tokio_runtime()).iter(|| async {
                let tasks = (0..*NUM_THREADS_SMALL)
                    .map(|_| tokio::task::spawn_blocking(|| fibonacci(FIB_N)))
                    .collect::<Vec<_>>();

                join_all(tasks)
                    .await
                    .into_iter()
                    .map(|res| res.unwrap())
                    .sum::<u64>();
            });
        },
    );

    c.bench_function("spawn small tokio blocking thread sleep", |b| {
        b.to_async(multi_thread_tokio_runtime()).iter(|| async {
            let tasks = (0..*NUM_THREADS_SMALL)
                .map(|_| {
                    tokio::task::spawn_blocking(|| {
                        std::thread::sleep(Duration::from_millis(SLEEP_MS))
                    })
                })
                .collect::<Vec<_>>();

            join_all(tasks)
                .await
                .into_iter()
                .map(|res| res.unwrap())
                .for_each(|_| {});
        });
    });

    c.bench_function(
        "spawn large tokio blocking thread expensive calculation",
        |b| {
            b.to_async(multi_thread_tokio_runtime()).iter(|| async {
                let tasks = (0..*NUM_THREADS_LARGE)
                    .map(|_| tokio::task::spawn_blocking(|| fibonacci(FIB_N)))
                    .collect::<Vec<_>>();

                join_all(tasks)
                    .await
                    .into_iter()
                    .map(|res| res.unwrap())
                    .sum::<u64>();
            });
        },
    );

    c.bench_function("spawn large tokio blocking thread sleep", |b| {
        b.to_async(multi_thread_tokio_runtime()).iter(|| async {
            let tasks = (0..*NUM_THREADS_LARGE)
                .map(|_| {
                    tokio::task::spawn_blocking(|| {
                        std::thread::sleep(Duration::from_millis(SLEEP_MS))
                    })
                })
                .collect::<Vec<_>>();

            join_all(tasks)
                .await
                .into_iter()
                .map(|res| res.unwrap())
                .for_each(|_| {});
        });
    });

    c.bench_function("spawn huge tokio blocking thread sleep", |b| {
        b.to_async(multi_thread_tokio_runtime()).iter(|| async {
            let tasks = (0..*NUM_THREADS_HUGE)
                .map(|_| {
                    tokio::task::spawn_blocking(|| {
                        std::thread::sleep(Duration::from_millis(SLEEP_MS))
                    })
                })
                .collect::<Vec<_>>();

            join_all(tasks)
                .await
                .into_iter()
                .map(|res| res.unwrap())
                .for_each(|_| {});
        });
    });

    c.bench_function(
        "spawn tokio blocking thread large worker huge sleep complex workload",
        |b| {
            b.to_async(multi_thread_tokio_runtime()).iter(|| async {
                let work_tasks = (0..*NUM_THREADS_LARGE)
                    .map(|_| {
                        tokio::task::spawn_blocking(|| {
                            fibonacci(FIB_N);
                        })
                    })
                    .collect::<Vec<_>>();

                let sleep_tasks = (0..*NUM_THREADS_HUGE)
                    .map(|_| {
                        tokio::task::spawn(async {
                            tokio::time::sleep(Duration::from_millis(SLEEP_MS)).await
                        })
                    })
                    .collect::<Vec<_>>();

                join_all(work_tasks.into_iter().chain(sleep_tasks))
                    .await
                    .into_iter()
                    .map(|res| res.unwrap())
                    .for_each(|_| {});
            })
        },
    );
}

fn instruction_benchmarks(c: &mut Criterion) {
    c.bench_function("instruction syscall getpid", |b| {
        b.iter(|| unsafe { syscalls::raw_syscall!(syscalls::Sysno::getpid) });
    });

    c.bench_function("instruction cpuid", |b| {
        b.iter(|| unsafe { __cpuid(0) });
    });

    c.bench_function("instruction nop", |b| {
        b.iter(|| unsafe { asm!("nop") });
    });

    c.bench_function("instruction nops", |b| {
        b.iter(|| unsafe { asm!("nop; nop; nop; nop; nop; nop; nop; nop; nop; nop;") });
    });

    c.bench_function("instruction empty", |b| {
        b.iter(|| unsafe { asm!("") });
    });
}

criterion_group!(
    benches,
    fib_benchmark,
    system_benchmark,
    tokio_benchmark,
    instruction_benchmarks
);
criterion_main!(benches);
