mod common;
use std::{
    cell::Cell,
    sync::{Arc, Barrier},
    thread,
    time::Duration,
};

use common::*;
use thin_cell::sync::*;

testcases!();

#[test]
fn test_send_sync_bounds() {
    fn assert_mt<T: Send + Sync>() {}

    assert_mt::<ThinCell<i32>>();
    assert_mt::<ThinCell<String>>();
    assert_mt::<ThinCell<[u8]>>();
    assert_mt::<ThinCell<dyn std::fmt::Debug + Send + Sync>>();
}

#[test]
fn test_simple_cross_thread_clone() {
    let cell = ThinCell::new(42);
    let cell_clone = cell.clone();

    let handle = thread::spawn(move || {
        assert_eq!(*cell_clone.borrow(), 42);
        *cell_clone.borrow() = 100;
    });

    handle.join().unwrap();
    assert_eq!(*cell.borrow(), 100);
}

#[test]
fn test_cross_thread_mutation() {
    let cell = ThinCell::new(0);
    let cell_clone = cell.clone();

    let handle = thread::spawn(move || {
        for i in 0..100 {
            let mut borrowed = cell_clone.borrow();
            *borrowed = i;
        }
    });

    handle.join().unwrap();
    let final_value = *cell.borrow();
    assert_eq!(final_value, 99);
}

#[test]
fn test_concurrent_cloning() {
    let cell = ThinCell::new(42);
    let mut handles = vec![];

    for _ in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let clones: Vec<_> = (0..10).map(|_| cell_clone.clone()).collect();
            assert_eq!(clones.len(), 10);

            for c in &clones {
                assert_eq!(*c.borrow(), 42);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), 42);
}

#[test]
fn test_concurrent_clone_and_drop() {
    let cell = ThinCell::new(vec![1, 2, 3, 4, 5]);
    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    for _ in 0..10 {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let _ = cell_clone.clone();
            assert_eq!(cell_clone.borrow().len(), 5);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_sequential_borrow_across_threads() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];

    for i in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(i * 10));
            let mut borrowed = cell_clone.borrow();
            *borrowed += 1;
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), 10);
}

#[test]
fn test_try_borrow_concurrent() {
    let cell = ThinCell::new(42);
    let cell_clone = cell.clone();
    let barrier = Arc::new(Barrier::new(2));
    let barrier_clone = barrier.clone();

    let handle = thread::spawn(move || {
        let borrowed = cell_clone.borrow();
        barrier_clone.wait();
        thread::sleep(Duration::from_millis(100));
        *borrowed
    });

    barrier.wait();

    assert!(cell.try_borrow().is_none());

    let result = handle.join().unwrap();
    assert_eq!(result, 42);

    assert!(cell.try_borrow().is_some());
}

#[test]
fn test_blocking_borrow() {
    let cell = ThinCell::new(0);
    let cell_clone = cell.clone();

    let handle = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        let mut borrowed = cell_clone.borrow();
        thread::sleep(Duration::from_millis(100));
        *borrowed = 42;
    });

    thread::sleep(Duration::from_millis(50));

    handle.join().unwrap();
    assert_eq!(*cell.borrow(), 42);
}

#[test]
fn test_many_threads_reading() {
    let cell = ThinCell::new(vec![1, 2, 3, 4, 5]);
    let barrier = Arc::new(Barrier::new(50));
    let mut handles = vec![];

    for _ in 0..50 {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            let borrowed = cell_clone.borrow();
            assert_eq!(borrowed.len(), 5);
            assert_eq!(borrowed[0], 1);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_many_threads_sequential_writing() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];
    let num_threads = 100;
    let increments_per_thread = 10;

    for _ in 0..num_threads {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            for _ in 0..increments_per_thread {
                let mut borrowed = cell_clone.borrow();
                *borrowed += 1;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), num_threads * increments_per_thread);
}

#[test]
fn test_contention_with_try_borrow() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];
    let num_threads = 20;
    let barrier = Arc::new(Barrier::new(num_threads));

    for _ in 0..num_threads {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            let mut success_count = 0;
            for _ in 0..100 {
                if let Some(mut borrowed) = cell_clone.try_borrow() {
                    *borrowed += 1;
                    success_count += 1;

                    thread::sleep(Duration::from_micros(10));
                } else {
                    thread::yield_now();
                }
            }
            success_count
        });
        handles.push(handle);
    }

    let total_successes: i32 = handles.into_iter().map(|h| h.join().unwrap()).sum();

    assert_eq!(*cell.borrow(), total_successes);
    assert!(total_successes > 0);
}

#[test]
fn test_rapid_borrow_release() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];

    for _ in 0..20 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            for _ in 0..1000 {
                let mut borrowed = cell_clone.borrow();
                *borrowed += 1;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert!(*cell.borrow() >= 19000);
}

#[test]
fn test_borrow_across_thread_lifetime() {
    let cell = ThinCell::new(vec![1, 2, 3]);
    let cell_clone = cell.clone();

    let handle = thread::spawn(move || {
        let borrowed = cell_clone.borrow();
        thread::sleep(Duration::from_millis(100));
        borrowed.iter().sum::<i32>()
    });

    thread::sleep(Duration::from_millis(50));

    let sum = handle.join().unwrap();
    assert_eq!(sum, 6);
}

#[test]
fn test_concurrent_vec_operations() {
    let cell = ThinCell::new(Vec::<i32>::new());
    let mut handles = vec![];

    for i in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_millis(i * 5));
            let mut borrowed = cell_clone.borrow();
            borrowed.push(i as i32);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let borrowed = cell.borrow();
    assert_eq!(borrowed.len(), 10);

    let mut sorted = borrowed.clone();
    sorted.sort();
    assert_eq!(sorted, vec![0, 1, 2, 3, 4, 5, 6, 7, 8, 9]);
}

#[test]
fn test_concurrent_string_operations() {
    let cell = ThinCell::new(String::new());
    let mut handles = vec![];

    for i in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let mut borrowed = cell_clone.borrow();
            borrowed.push_str(&format!("{}", i));
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let borrowed = cell.borrow();
    assert_eq!(borrowed.len(), 10);
}

#[test]
fn test_concurrent_count_tracking() {
    let cell = ThinCell::new(42);
    let barrier = Arc::new(Barrier::new(11));
    let mut handles = vec![];

    for _ in 0..10 {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let count = cell_clone.count();
            assert!(count >= 2, "Count was {}", count);
        });
        handles.push(handle);
    }

    barrier.wait();

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(cell.count(), 1);
}

#[test]
fn test_count_during_concurrent_operations() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];

    for _ in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                let _count_before = cell_clone.count();
                let mut borrowed = cell_clone.borrow();
                *borrowed += 1;
                let count_after = cell_clone.count();

                assert!(count_after >= 1);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_clone_count_stability() {
    let cell = ThinCell::new(42);
    let barrier = Arc::new(Barrier::new(21));
    let mut handles = vec![];

    for _ in 0..20 {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();

            let count1 = cell_clone.count();
            thread::sleep(Duration::from_micros(100));
            let count2 = cell_clone.count();

            assert!(count1 >= 1 && count2 >= 1);
        });
        handles.push(handle);
    }

    barrier.wait();

    for handle in handles {
        handle.join().unwrap();
    }
}

#[test]
fn test_concurrent_drop() {
    static DROP_COUNT: std::sync::atomic::AtomicUsize = std::sync::atomic::AtomicUsize::new(0);

    struct DropCounter;
    impl Drop for DropCounter {
        fn drop(&mut self) {
            DROP_COUNT.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }
    }

    DROP_COUNT.store(0, std::sync::atomic::Ordering::SeqCst);

    let cell = ThinCell::new(DropCounter);
    let mut handles = vec![];

    for _ in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let _ = cell_clone;
        });
        handles.push(handle);
    }

    drop(cell);

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(DROP_COUNT.load(std::sync::atomic::Ordering::SeqCst), 1);
}

#[test]
fn test_drop_while_borrowed_in_other_thread() {
    let cell = ThinCell::new(vec![1, 2, 3]);
    let cell_clone = cell.clone();
    let barrier = Arc::new(Barrier::new(2));
    let barrier_clone = barrier.clone();

    let handle = thread::spawn(move || {
        let borrowed = cell_clone.borrow();
        barrier_clone.wait();
        thread::sleep(Duration::from_millis(100));
        assert_eq!(borrowed.len(), 3);
    });

    barrier.wait();

    drop(cell);

    handle.join().unwrap();
}

#[test]
fn test_try_unwrap_single_owner() {
    let cell = ThinCell::new(vec![1, 2, 3, 4, 5]);

    let result = cell.try_unwrap();
    assert!(result.is_ok());
    assert_eq!(result.unwrap(), vec![1, 2, 3, 4, 5]);
}

#[test]
fn test_try_unwrap_multiple_owners() {
    let cell = ThinCell::new(vec![1, 2, 3]);
    let cell_clone = cell.clone();

    let result = cell.try_unwrap();
    assert!(result.is_err());

    drop(cell_clone);
}

#[test]
fn test_try_unwrap_while_borrowed() {
    let cell = ThinCell::new(42);
    {
        let _borrowed = cell.borrow();
        let cell_clone = cell.clone();
        let result = cell_clone.try_unwrap();
        assert!(result.is_err());
    }
}

#[test]
fn test_concurrent_try_unwrap() {
    let cell = ThinCell::new(100);
    let cell_clone = cell.clone();
    let barrier = Arc::new(Barrier::new(2));
    let barrier_clone = barrier.clone();

    let handle = thread::spawn(move || {
        barrier_clone.wait();
        thread::sleep(Duration::from_millis(50));
        drop(cell_clone);
    });

    barrier.wait();

    let result1 = cell.try_unwrap();
    assert!(result1.is_err());
    let cell = result1.unwrap_err();

    handle.join().unwrap();

    thread::sleep(Duration::from_millis(100));
    let result2 = cell.try_unwrap();
    assert!(result2.is_ok());
    assert_eq!(result2.unwrap(), 100);
}

#[test]
fn test_producer_consumer_pattern() {
    let cell = ThinCell::new(Vec::<i32>::new());
    let cell_producer = cell.clone();
    let cell_consumer = cell.clone();

    let producer = thread::spawn(move || {
        for i in 0..100 {
            let mut borrowed = cell_producer.borrow();
            borrowed.push(i);
        }
    });

    let consumer = thread::spawn(move || {
        loop {
            let borrowed = cell_consumer.borrow();
            if borrowed.len() == 100 {
                let sum: i32 = borrowed.iter().sum();
                return sum;
            }
            drop(borrowed);
            thread::yield_now();
        }
    });

    producer.join().unwrap();
    let sum = consumer.join().unwrap();
    assert_eq!(sum, (0..100).sum());
}

#[test]
fn test_round_robin_access() {
    let cell = ThinCell::new(0);
    let num_threads = 10;
    let iterations = 100;
    let mut handles = vec![];

    for thread_id in 0..num_threads {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            for _iteration in 0..iterations {
                loop {
                    if let Some(mut borrowed) = cell_clone.try_borrow()
                        && *borrowed % num_threads == thread_id
                    {
                        *borrowed += 1;
                        break;
                    }
                    thread::yield_now();
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), num_threads * iterations);
}

#[test]
fn test_cascading_clones() {
    let cell = ThinCell::new(42);
    let mut handles = vec![];

    for depth in 1..=5 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let mut current = cell_clone;
            for _ in 0..depth {
                current = current.clone();
            }
            *current.borrow()
        });
        handles.push(handle);
    }

    for handle in handles {
        assert_eq!(handle.join().unwrap(), 42);
    }
}

#[test]
fn test_interleaved_borrow_and_clone() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];

    for _i in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            for _ in 0..50 {
                let new_clone = cell_clone.clone();

                let mut borrowed = new_clone.borrow();
                *borrowed += 1;
                drop(borrowed);

                drop(new_clone);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), 500);
}

#[test]
fn test_concurrent_downcast() {
    use std::any::Any;

    trait Foo: Any {}

    #[derive(Debug)]
    struct Bar {}

    impl Foo for Bar {}

    // Create a ThinCell with a trait object
    let cell_dyn = unsafe { ThinCell::<dyn Foo>::new_unsize(Bar {}, |p| p as _) };

    // upcast
    let cell_any = unsafe { cell_dyn.unsize::<dyn Any>(|p| p as _) };

    // downcast
    cell_any.clone().downcast::<Bar>().unwrap();

    // downcast type mismatch (should fail with Type error)
    let result = cell_any.clone().downcast::<String>();
    assert!(matches!(result, Err(DowncastError::Type(_))));

    let r = result.unwrap_err().into_inner();
    let _g = r.borrow();

    let result = cell_any.downcast::<Bar>();
    assert!(matches!(result, Err(DowncastError::Borrowed(_))));
}

#[test]
fn test_concurrent_slice_access() {
    let cell = ThinCell::new([1, 2, 3, 4, 5, 6, 7, 8, 9, 10]);
    let mut handles = vec![];

    for i in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let mut borrowed = cell_clone.borrow();
            borrowed[i] = i as i32 * 10;
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let borrowed = cell.borrow();
    for i in 0..10 {
        assert_eq!(borrowed[i], i as i32 * 10);
    }
}

#[test]
fn test_mixed_sized_unsized_operations() {
    let cell = ThinCell::new([1, 2, 3, 4, 5]);
    let cell_clone = cell.clone();

    let handle = thread::spawn(move || cell_clone.borrow().len());

    assert_eq!(handle.join().unwrap(), 5);
}

#[test]
fn test_extreme_contention() {
    let cell = ThinCell::new(0);
    let num_threads = 10;
    let operations_per_thread = 100;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for _ in 0..num_threads {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            for _ in 0..operations_per_thread {
                let mut borrowed = cell_clone.borrow();
                *borrowed += 1;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let result = *cell.borrow();
    assert_eq!(result, num_threads * operations_per_thread);
}

#[test]
fn test_alternating_readers_writers() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];

    for i in 0..20 {
        let cell_clone = cell.clone();
        if i % 2 == 0 {
            let handle = thread::spawn(move || {
                let mut borrowed = cell_clone.borrow();
                *borrowed += 1;
                thread::sleep(Duration::from_micros(100));
            });
            handles.push(handle);
        } else {
            let handle = thread::spawn(move || {
                let borrowed = cell_clone.borrow();
                let _value = *borrowed;
                thread::sleep(Duration::from_micros(100));
            });
            handles.push(handle);
        }
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), 10);
}

#[test]
fn test_no_use_after_free() {
    let cell = ThinCell::new(vec![1, 2, 3, 4, 5]);
    let cell_clone = cell.clone();

    let handle = thread::spawn(move || {
        let borrowed = cell_clone.borrow();
        thread::sleep(Duration::from_millis(50));
        borrowed.len()
    });

    drop(cell);

    assert_eq!(handle.join().unwrap(), 5);
}

#[test]
fn test_concurrent_ptr_eq() {
    let cell1 = ThinCell::new(42);
    let cell2 = cell1.clone();
    let cell3 = ThinCell::new(42);

    let handle = thread::spawn(move || {
        assert!(cell1.ptr_eq(&cell2));
        assert!(!cell1.ptr_eq(&cell3));
    });

    handle.join().unwrap();
}

#[test]
fn test_concurrent_leak_and_from_raw() {
    let cell = ThinCell::new(vec![1, 2, 3]);
    let ptr = cell.leak() as usize;

    let handle = thread::spawn(move || {
        let recovered: ThinCell<Vec<i32>> = unsafe { ThinCell::from_raw(ptr as _) };
        recovered.borrow().len()
    });

    assert_eq!(handle.join().unwrap(), 3);
}

#[test]
fn test_debug_display_concurrent() {
    let cell = ThinCell::new(42);
    let cell_clone = cell.clone();

    let handle = thread::spawn(move || {
        let debug_str = format!("{:?}", cell_clone);
        assert!(debug_str.contains("42") || debug_str.contains("borrowed"));
        let display_str = format!("{}", cell_clone);
        assert!(display_str.contains("42") || display_str.contains("borrowed"));
    });

    handle.join().unwrap();
}

#[test]
fn test_ping_pong_pattern() {
    let cell = ThinCell::new((0, false));
    let cell_ping = cell.clone();
    let cell_pong = cell.clone();
    let barrier = Arc::new(Barrier::new(2));
    let barrier_ping = barrier.clone();
    let barrier_pong = barrier.clone();

    let ping = thread::spawn(move || {
        barrier_ping.wait();
        for i in 0..100 {
            loop {
                if let Some(mut borrowed) = cell_ping.try_borrow()
                    && !borrowed.1
                {
                    borrowed.0 = i;
                    borrowed.1 = true;
                    break;
                }
                thread::yield_now();
            }
        }
    });

    let pong = thread::spawn(move || {
        barrier_pong.wait();
        for i in 0..100 {
            loop {
                if let Some(mut borrowed) = cell_pong.try_borrow()
                    && borrowed.1
                {
                    assert_eq!(borrowed.0, i);
                    borrowed.1 = false;
                    break;
                }
                thread::yield_now();
            }
        }
    });

    ping.join().unwrap();
    pong.join().unwrap();
}

#[test]
fn test_multi_way_ping_pong() {
    let cell = ThinCell::new(0);
    let num_threads = 10;
    let passes = 50;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for id in 0..num_threads {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            for _ in 0..passes {
                loop {
                    if let Some(mut borrowed) = cell_clone.try_borrow()
                        && *borrowed % num_threads == id
                    {
                        *borrowed += 1;
                        break;
                    }
                    thread::yield_now();
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), num_threads * passes);
}

#[test]
fn test_chained_borrows_across_threads() {
    let cell1 = ThinCell::new(1);
    let cell2 = ThinCell::new(2);
    let cell3 = ThinCell::new(3);

    let c1 = cell1.clone();
    let c2 = cell2.clone();
    let c3 = cell3.clone();

    let handle = thread::spawn(move || {
        let mut b1 = c1.borrow();
        *b1 += 10;
        drop(b1);

        let mut b2 = c2.borrow();
        *b2 += 20;
        drop(b2);

        let mut b3 = c3.borrow();
        *b3 += 30;
        drop(b3);
    });

    handle.join().unwrap();

    assert_eq!(*cell1.borrow(), 11);
    assert_eq!(*cell2.borrow(), 22);
    assert_eq!(*cell3.borrow(), 33);
}

#[test]
fn test_dependency_chain() {
    let cell1 = ThinCell::new(0);
    let cell2 = ThinCell::new(0);
    let cell3 = ThinCell::new(0);

    let c1_t1 = cell1.clone();
    let c2_t1 = cell2.clone();

    let c2_t2 = cell2.clone();
    let c3_t2 = cell3.clone();

    let barrier1 = Arc::new(Barrier::new(2));
    let barrier2 = Arc::new(Barrier::new(2));
    let b1_t1 = barrier1.clone();
    let b2_t2 = barrier2.clone();

    let t1 = thread::spawn(move || {
        *c1_t1.borrow() = 42;
        b1_t1.wait();
        thread::sleep(Duration::from_millis(10));
        *c2_t1.borrow() = 100;
    });

    let t2 = thread::spawn(move || {
        barrier1.wait();
        thread::sleep(Duration::from_millis(50));
        let val = *c2_t2.borrow();
        b2_t2.wait();
        *c3_t2.borrow() = val + 10;
    });

    t1.join().unwrap();
    barrier2.wait();
    t2.join().unwrap();

    assert_eq!(*cell1.borrow(), 42);
    assert_eq!(*cell2.borrow(), 100);
    assert_eq!(*cell3.borrow(), 110);
}

#[test]
fn test_panic_during_borrow() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let cell = ThinCell::new(vec![1, 2, 3]);
    let cell_clone = cell.clone();

    let result = catch_unwind(AssertUnwindSafe(|| {
        let mut borrowed = cell_clone.borrow();
        borrowed.push(4);
        panic!("Intentional panic");
    }));

    assert!(result.is_err());

    let borrowed = cell.borrow();
    assert_eq!(borrowed.len(), 4);
}

#[test]
fn test_panic_in_one_thread_doesnt_affect_others() {
    use std::panic::{AssertUnwindSafe, catch_unwind};

    let cell = ThinCell::new(0);
    let cell_panic = cell.clone();
    let cell_normal = cell.clone();

    let panic_handle = thread::spawn(move || {
        catch_unwind(AssertUnwindSafe(|| {
            let mut borrowed = cell_panic.borrow();
            *borrowed = 42;
            panic!("Panic in thread");
        }))
    });

    thread::sleep(Duration::from_millis(50));

    let normal_handle = thread::spawn(move || {
        *cell_normal.borrow() = 100;
    });

    let _ = panic_handle.join();
    normal_handle.join().unwrap();

    assert_eq!(*cell.borrow(), 100);
}

#[test]
fn test_worker_pool_pattern() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let cell = ThinCell::new(Vec::<i32>::new());
    let done = Arc::new(AtomicBool::new(false));
    let num_workers = 10;
    let mut handles = vec![];

    for worker_id in 0..num_workers {
        let cell_clone = cell.clone();
        let done_clone = done.clone();
        let handle = thread::spawn(move || {
            let mut count = 0;
            while !done_clone.load(Ordering::Relaxed) {
                if let Some(mut borrowed) = cell_clone.try_borrow() {
                    borrowed.push(worker_id);
                    count += 1;
                    thread::sleep(Duration::from_micros(100));
                } else {
                    thread::yield_now();
                }
            }
            count
        });
        handles.push(handle);
    }

    thread::sleep(Duration::from_millis(200));
    done.store(true, Ordering::Relaxed);

    let total_work: i32 = handles.into_iter().map(|h| h.join().unwrap()).sum();
    let final_len = cell.borrow().len();

    assert_eq!(final_len, total_work as usize);
    assert!(final_len > 0);
}

#[test]
fn test_barrier_synchronized_operations() {
    let cell = ThinCell::new(0);
    let num_threads = 20;
    let barrier = Arc::new(Barrier::new(num_threads));
    let mut handles = vec![];

    for _ in 0..num_threads {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();

            *cell_clone.borrow() += 1;

            barrier_clone.wait();

            *cell_clone.borrow()
        });
        handles.push(handle);
    }

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert!(results.iter().all(|&v| v == num_threads));
}

#[test]
fn test_scoped_thread_lifetime() {
    let cell = ThinCell::new(42);

    std::thread::scope(|s| {
        for _ in 0..10 {
            let cell_ref = &cell;
            s.spawn(move || {
                let mut borrowed = cell_ref.borrow();
                *borrowed += 1;
            });
        }
    });

    assert_eq!(*cell.borrow(), 52);
}

#[test]
fn test_hierarchical_thread_tree() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];

    for _ in 0..5 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let mut child_handles = vec![];
            for _ in 0..4 {
                let cell_child = cell_clone.clone();
                let child = thread::spawn(move || {
                    *cell_child.borrow() += 1;
                });
                child_handles.push(child);
            }
            for child in child_handles {
                child.join().unwrap();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), 20);
}

#[test]
fn test_no_starvation() {
    const THREADS: usize = 8;

    let cell = ThinCell::new(0);
    let barrier = Arc::new(Barrier::new(THREADS));
    let mut handles = vec![];

    for _ in 0..THREADS {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            let mut count = 0;
            let start = std::time::Instant::now();
            while start.elapsed() < Duration::from_millis(100) {
                let mut borrowed = cell_clone.borrow();
                *borrowed += 1;
                count += 1;
            }
            count
        });
        handles.push(handle);
    }

    let res = handles
        .into_iter()
        .map(|x| x.join().unwrap())
        .collect::<Vec<_>>();

    let min = *res.iter().min().unwrap();
    let max = *res.iter().max().unwrap();

    assert!(max <= min * 10 || min == 0, "min: {}, max: {}", min, max);

    println!("min: {min}, max: {max}");
}

#[test]
fn test_concurrent_clone_during_writes() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];

    for _ in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            for _ in 0..200 {
                let mut borrowed = cell_clone.borrow();
                *borrowed += 1;
            }
        });
        handles.push(handle);
    }

    let cell_clone = cell.clone();
    let cloner = thread::spawn(move || {
        let mut clones = vec![];
        for _ in 0..50 {
            clones.push(cell_clone.clone());
            thread::yield_now();
        }
        clones.len()
    });

    for handle in handles {
        handle.join().unwrap();
    }
    let clone_count = cloner.join().unwrap();

    assert_eq!(*cell.borrow(), 2000);
    assert_eq!(clone_count, 50);
}

#[test]
fn test_massive_thread_spawn() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];

    for _ in 0..100 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            for _ in 0..10 {
                let mut borrowed = cell_clone.borrow();
                *borrowed += 1;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), 1000);
}

#[test]
fn test_long_held_borrows() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];

    for i in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let mut borrowed = cell_clone.borrow();
            thread::sleep(Duration::from_millis(50));
            *borrowed = i;
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_val = *cell.borrow();
    assert!(final_val < 10);
}

#[test]
fn test_mixed_operation_stress() {
    let cell = ThinCell::new(Vec::<i32>::new());
    let mut handles = vec![];

    for i in 0..20 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let mut pushes = 0;
            while pushes < 50 {
                if let Some(mut borrowed) = cell_clone.try_borrow() {
                    borrowed.push(i);
                    pushes += 1;
                } else {
                    thread::yield_now();
                }
            }
        });
        handles.push(handle);
    }

    for _ in 0..5 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let mut max_len = 0;
            for _ in 0..100 {
                if let Some(borrowed) = cell_clone.try_borrow() {
                    max_len = max_len.max(borrowed.len());
                }
                thread::yield_now();
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_len = cell.borrow().len();
    assert_eq!(final_len, 1000);
}

#[test]
fn test_recursive_clone_tree() {
    fn spawn_tree(cell: ThinCell<i32>, depth: usize) -> thread::JoinHandle<()> {
        thread::spawn(move || {
            *cell.borrow() += 1;
            if depth > 0 {
                let left = spawn_tree(cell.clone(), depth - 1);
                let right = spawn_tree(cell.clone(), depth - 1);
                left.join().unwrap();
                right.join().unwrap();
            }
        })
    }

    let cell = ThinCell::new(0);
    let handle = spawn_tree(cell.clone(), 3);
    handle.join().unwrap();

    assert_eq!(*cell.borrow(), 15);
}

#[test]
fn test_alternating_try_borrow_and_borrow() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];

    for i in 0..20 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                if i % 2 == 0 {
                    if let Some(mut borrowed) = cell_clone.try_borrow() {
                        *borrowed += 1;
                    }
                } else {
                    let mut borrowed = cell_clone.borrow();
                    *borrowed += 1;
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_val = *cell.borrow();
    assert!(final_val > 0);
    assert!(final_val <= 2000);
}

#[test]
fn test_sequential_consistency() {
    let cell = ThinCell::new(0);
    let barrier = Arc::new(Barrier::new(2));
    let barrier_clone = barrier.clone();
    let cell_writer = cell.clone();
    let cell_reader = cell.clone();

    let writer = thread::spawn(move || {
        *cell_writer.borrow() = 42;
        barrier_clone.wait();
    });

    let reader = thread::spawn(move || {
        barrier.wait();
        *cell_reader.borrow()
    });

    writer.join().unwrap();
    let value = reader.join().unwrap();
    assert_eq!(value, 42);
}

#[test]
fn test_multiple_writer_visibility() {
    let cell = ThinCell::new(vec![0; 100]);
    let mut handles = vec![];

    for i in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let mut borrowed = cell_clone.borrow();
            for j in 0..10 {
                borrowed[i * 10 + j] = i;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let borrowed = cell.borrow();
    for i in 0..10 {
        for j in 0..10 {
            assert_eq!(borrowed[i * 10 + j], i);
        }
    }
}

#[test]
fn test_empty_string_concurrent() {
    let cell = ThinCell::new(String::new());
    let mut handles = vec![];

    for i in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let mut borrowed = cell_clone.borrow();
            borrowed.push_str(&i.to_string());
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(cell.borrow().len(), 10);
}

#[test]
fn test_option_concurrent_mutations() {
    let cell = ThinCell::new(Some(0));
    let mut handles = vec![];

    for i in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let mut borrowed = cell_clone.borrow();
            if let Some(ref mut val) = *borrowed {
                *val += i;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), Some(45));
}

#[test]
fn test_tuple_concurrent_access() {
    let cell = ThinCell::new((0, String::new(), vec![0u8; 0]));
    let mut handles = vec![];

    for i in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let mut borrowed = cell_clone.borrow();
            borrowed.0 += i;
            borrowed.1.push('x');
            borrowed.2.push(i as u8);
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let borrowed = cell.borrow();
    assert_eq!(borrowed.0, 45);
    assert_eq!(borrowed.1.len(), 10);
    assert_eq!(borrowed.2.len(), 10);
}

#[test]
fn test_box_trait_object_concurrent() {
    let cell = ThinCell::new(Box::new(42) as Box<dyn std::any::Any + Send + Sync>);
    let cell_clone = cell.clone();

    let handle = thread::spawn(move || {
        let borrowed = cell_clone.borrow();
        borrowed.downcast_ref::<i32>().copied()
    });

    let result = handle.join().unwrap();
    assert_eq!(result, Some(42));
}

#[test]
fn test_high_frequency_operations() {
    let cell = ThinCell::new(0u64);
    let num_threads = 4;
    let ops_per_thread = 10000;
    let mut handles = vec![];

    for _ in 0..num_threads {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            for _ in 0..ops_per_thread {
                let mut borrowed = cell_clone.borrow();
                *borrowed += 1;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), (num_threads * ops_per_thread) as u64);
}

#[test]
fn test_complete_lifecycle() {
    let cell = ThinCell::new(vec![1, 2, 3]);
    assert_eq!(cell.count(), 1);

    let cell2 = cell.clone();
    assert_eq!(cell.count(), 2);

    let cell3 = cell.clone();
    let handle = thread::spawn(move || cell3.borrow().len());
    assert_eq!(handle.join().unwrap(), 3);

    {
        let mut borrowed = cell.borrow();
        borrowed.push(4);
    }

    assert_eq!(cell2.borrow().len(), 4);

    drop(cell2);
    assert_eq!(cell.count(), 1);

    assert_eq!(*cell.borrow(), vec![1, 2, 3, 4]);
}

#[test]
fn test_complex_multi_phase_operation() {
    let cell = ThinCell::new(0);
    let barrier1 = Arc::new(Barrier::new(11));
    let barrier2 = Arc::new(Barrier::new(11));
    let mut handles = vec![];

    for id in 0..10 {
        let cell_clone = cell.clone();
        let b1 = barrier1.clone();
        let b2 = barrier2.clone();
        let handle = thread::spawn(move || {
            b1.wait();

            *cell_clone.borrow() += id;
            b2.wait();

            *cell_clone.borrow()
        });
        handles.push(handle);
    }

    barrier1.wait();
    barrier2.wait();

    let results: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();

    assert!(results.windows(2).all(|w| w[0] == w[1]));

    assert_eq!(results[0], 45);
}

#[test]
fn test_spinlock_under_contention() {
    let cell = ThinCell::new(0);
    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    for _ in 0..10 {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            for _ in 0..100 {
                let mut borrowed = cell_clone.borrow();
                *borrowed += 1;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), 1000);
}

#[test]
fn test_yield_during_contention() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    let cell = ThinCell::new(0);
    let attempts = Arc::new(AtomicUsize::new(0));
    let mut handles = vec![];

    for _ in 0..5 {
        let cell_clone = cell.clone();
        let attempts_clone = attempts.clone();
        let handle = thread::spawn(move || {
            for _ in 0..100 {
                loop {
                    attempts_clone.fetch_add(1, Ordering::Relaxed);
                    if let Some(mut borrowed) = cell_clone.try_borrow() {
                        *borrowed += 1;
                        break;
                    }
                    thread::yield_now();
                }
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), 500);
    let total_attempts = attempts.load(Ordering::Relaxed);
    assert!(total_attempts >= 500);
}

#[test]
fn test_configuration_update_pattern() {
    #[derive(Clone, Debug, PartialEq)]
    struct Config {
        timeout: u64,
        max_connections: usize,
        enabled: bool,
    }

    let config = ThinCell::new(Config {
        timeout: 1000,
        max_connections: 100,
        enabled: true,
    });

    let reader_cells: Vec<_> = (0..10).map(|_| config.clone()).collect();
    let writer_cell = config.clone();

    let mut handles = vec![];
    for cell in reader_cells {
        let handle = thread::spawn(move || {
            for _ in 0..50 {
                let borrowed = cell.borrow();
                let _ = borrowed.timeout;
                thread::sleep(Duration::from_micros(10));
            }
        });
        handles.push(handle);
    }

    let writer = thread::spawn(move || {
        thread::sleep(Duration::from_millis(10));
        let mut borrowed = writer_cell.borrow();
        borrowed.timeout = 2000;
        borrowed.max_connections = 200;
    });

    for handle in handles {
        handle.join().unwrap();
    }
    writer.join().unwrap();

    let final_config = config.borrow();
    assert_eq!(final_config.timeout, 2000);
    assert_eq!(final_config.max_connections, 200);
}

#[test]
fn test_clone_during_high_contention() {
    let cell = ThinCell::new(0);
    let barrier = Arc::new(Barrier::new(20));
    let mut handles = vec![];

    for _ in 0..20 {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            let clones: Vec<_> = (0..10).map(|_| cell_clone.clone()).collect();
            for c in clones {
                *c.borrow() += 1;
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(*cell.borrow(), 200);
}

#[test]
fn test_reference_count_accuracy() {
    let cell = ThinCell::new(42);
    assert_eq!(cell.count(), 1);

    let clones: Vec<_> = (0..10).map(|_| cell.clone()).collect();
    assert_eq!(cell.count(), 11);

    drop(clones);
    assert_eq!(cell.count(), 1);
}

#[test]
fn test_concurrent_count_observations() {
    let cell = ThinCell::new(0);
    let barrier = Arc::new(Barrier::new(10));
    let mut handles = vec![];

    for _ in 0..10 {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();
            let count = cell_clone.count();
            assert!(count >= 2);
            count
        });
        handles.push(handle);
    }

    let counts: Vec<_> = handles.into_iter().map(|h| h.join().unwrap()).collect();
    assert!(counts.iter().all(|&c| c >= 2));
}

#[test]
fn test_maximum_practical_clones() {
    let cell = ThinCell::new(42);
    let clones: Vec<_> = (0..1000).map(|_| cell.clone()).collect();

    assert_eq!(cell.count(), 1001);

    let mut handles = vec![];
    for i in 0..10 {
        let clone = clones[i * 100].clone();
        let handle = thread::spawn(move || *clone.borrow());
        handles.push(handle);
    }

    for handle in handles {
        assert_eq!(handle.join().unwrap(), 42);
    }
}

#[test]
fn test_deep_clone_chain() {
    let mut current = ThinCell::new(0);
    for _ in 0..100 {
        current = current.clone();
    }

    let handle = thread::spawn(move || {
        *current.borrow() = 42;
    });

    handle.join().unwrap();
}

#[test]
fn test_thread_safety_invariants() {
    let cell = ThinCell::new(Vec::<usize>::new());
    let mut handles = vec![];

    for thread_id in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            for i in 0..100 {
                let mut borrowed = cell_clone.borrow();

                borrowed.push(thread_id * 1000 + i);
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_vec = cell.borrow();
    assert_eq!(final_vec.len(), 1000);

    for &val in final_vec.iter() {
        let thread_id = val / 1000;
        let offset = val % 1000;
        assert!(thread_id < 10);
        assert!(offset < 100);
    }
}

#[test]
fn test_happens_before_relationship() {
    use std::sync::atomic::{AtomicBool, Ordering};

    let cell = ThinCell::new(0);
    let flag = Arc::new(AtomicBool::new(false));
    let cell_writer = cell.clone();
    let cell_reader = cell.clone();
    let flag_writer = flag.clone();

    let writer = thread::spawn(move || {
        *cell_writer.borrow() = 42;
        flag_writer.store(true, Ordering::Release);
    });

    let reader = thread::spawn(move || {
        while !flag.load(Ordering::Acquire) {
            thread::yield_now();
        }
        *cell_reader.borrow()
    });

    writer.join().unwrap();
    let value = reader.join().unwrap();
    assert_eq!(value, 42);
}

#[test]
fn test_condvar_like_pattern() {
    let cell = ThinCell::new((false, 0));
    let cell_waiter = cell.clone();
    let cell_notifier = cell.clone();

    let waiter = thread::spawn(move || {
        loop {
            let borrowed = cell_waiter.borrow();
            if borrowed.0 {
                return borrowed.1;
            }
            drop(borrowed);
            thread::yield_now();
        }
    });

    thread::sleep(Duration::from_millis(50));

    let notifier = thread::spawn(move || {
        let mut borrowed = cell_notifier.borrow();
        borrowed.1 = 42;
        borrowed.0 = true;
    });

    notifier.join().unwrap();
    assert_eq!(waiter.join().unwrap(), 42);
}

#[test]
fn test_resilience_to_thread_termination() {
    let cell = ThinCell::new(vec![1, 2, 3]);

    let mut handles = vec![];
    for (i, cell_clone) in (0..10).map(|_| cell.clone()).enumerate() {
        let handle = thread::spawn(move || {
            if i % 3 == 0 {
                return;
            }
            let mut borrowed = cell_clone.borrow();
            borrowed.push(i);
        });
        handles.push(handle);
    }

    for handle in handles {
        let _ = handle.join();
    }

    let borrowed = cell.borrow();
    assert_eq!(borrowed.len(), 9);
}

#[test]
fn test_ultimate_stress_test() {
    let cell = ThinCell::new((0, Vec::<i32>::new(), String::new()));
    let barrier = Arc::new(Barrier::new(50));
    let mut handles = vec![];

    for i in 0..50 {
        let cell_clone = cell.clone();
        let barrier_clone = barrier.clone();
        let handle = thread::spawn(move || {
            barrier_clone.wait();

            for _ in 0..20 {
                let mut borrowed = cell_clone.borrow();
                borrowed.0 += 1;
                borrowed.1.push(i);
                borrowed.2.push('x');
            }
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let borrowed = cell.borrow();

    assert_eq!(borrowed.0, 1000);
    assert_eq!(borrowed.1.len(), 1000);
}

#[test]
fn test_drop_order_independence() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static DROP_ORDER: AtomicUsize = AtomicUsize::new(0);

    struct OrderTracker(usize);
    impl Drop for OrderTracker {
        fn drop(&mut self) {
            DROP_ORDER.store(self.0, Ordering::SeqCst);
        }
    }

    DROP_ORDER.store(0, Ordering::SeqCst);

    let cell = ThinCell::new(OrderTracker(42));
    let clones: Vec<_> = (0..10).map(|_| cell.clone()).collect();

    let mut handles = vec![];
    for (i, clone) in clones.into_iter().enumerate() {
        let handle = thread::spawn(move || {
            thread::sleep(Duration::from_micros(i as u64 * 10));
            drop(clone);
        });
        handles.push(handle);
    }

    drop(cell);

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(DROP_ORDER.load(Ordering::SeqCst), 42);
}

#[test]
fn test_clone_drop_interleaved() {
    let cell = ThinCell::new(0);
    let mut handles = vec![];

    for i in 0..10 {
        let cell_clone = cell.clone();
        let handle = thread::spawn(move || {
            let _temp_clones: Vec<_> = (0..5).map(|_| cell_clone.clone()).collect();
            thread::sleep(Duration::from_micros(i * 10));
        });
        handles.push(handle);
    }

    for handle in handles {
        handle.join().unwrap();
    }

    assert_eq!(cell.count(), 1);
}

#[test]
fn test_default_initialization() {
    let cell: ThinCell<Vec<i32>> = ThinCell::default();
    let cell_clone = cell.clone();

    let handle = thread::spawn(move || cell_clone.borrow().is_empty());

    assert!(handle.join().unwrap());
}

#[test]
fn test_comparison_operators_concurrent() {
    let cell1 = ThinCell::new(42);
    let cell2 = ThinCell::new(42);
    let cell3 = ThinCell::new(99);

    let c1 = cell1.clone();
    let c2 = cell2.clone();
    let c3 = cell3.clone();

    let handle = thread::spawn(move || (c1 == c2, c1 == c3));

    let (eq1, eq2) = handle.join().unwrap();
    assert!(eq1);
    assert!(!eq2);
}

#[test]
fn test_unsize_operations_concurrent() {
    let cell: ThinCell<[u8]> = unsafe { ThinCell::new_unsize([1u8, 2, 3, 4, 5], |ptr| ptr as _) };
    let cell_array = cell.clone();

    let handle1 = thread::spawn(move || cell_array.borrow()[0]);

    let cell_clone = cell.clone();
    let handle2 = thread::spawn(move || cell_clone.borrow().len());

    assert_eq!(handle1.join().unwrap(), 1);
    assert_eq!(handle2.join().unwrap(), 5);
}
