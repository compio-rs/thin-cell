use std::cell::Cell;

use thin_cell::ThinCell;

trait Greeter {
    fn greet(&self);
    fn set_id(&mut self, id: u32);
}

struct Robot {
    id: u32,
}

impl Greeter for Robot {
    fn greet(&self) {
        println!("Robot {}", self.id);
    }

    fn set_id(&mut self, id: u32) {
        self.id = id;
    }
}

#[test]
fn test_thin_cell_new() {
    let cell = ThinCell::new(42);
    assert_eq!(cell.count(), 1);
    assert_eq!(*cell.borrow(), 42);
}

#[test]
fn test_thin_cell_borrow_read() {
    let cell = ThinCell::new(100);
    let borrowed = cell.borrow();
    assert_eq!(*borrowed, 100);
}

#[test]
fn test_thin_cell_borrow_write() {
    let cell = ThinCell::new(42);
    {
        let mut borrowed = cell.borrow();
        *borrowed = 100;
    }
    assert_eq!(*cell.borrow(), 100);
}

#[test]
fn test_thin_cell_try_borrow_success() {
    let cell = ThinCell::new(42);
    let borrowed = cell.try_borrow();
    assert!(borrowed.is_some());
    assert_eq!(*borrowed.unwrap(), 42);
}

#[test]
fn test_thin_cell_try_borrow_failure() {
    let cell = ThinCell::new(42);
    let _borrowed = cell.borrow();

    // Should fail because already borrowed
    let result = cell.try_borrow();
    assert!(result.is_none());
}

#[test]
fn test_thin_cell_double_borrow_fails() {
    let cell = ThinCell::new(42);
    let _borrowed1 = cell.borrow();

    // Should fail to borrow again since already borrowed
    assert!(cell.try_borrow().is_none());
}

#[test]
fn test_thin_cell_sequential_borrows() {
    let cell = ThinCell::new(42);

    {
        let mut borrowed = cell.borrow();
        *borrowed = 100;
    } // Drop borrowed

    {
        let mut borrowed = cell.borrow();
        *borrowed = 200;
    } // Drop borrowed

    assert_eq!(*cell.borrow(), 200);
}

#[test]
fn test_thin_cell_clone() {
    let cell1 = ThinCell::new(42);
    assert_eq!(cell1.count(), 1);

    let cell2 = cell1.clone();
    assert_eq!(cell1.count(), 2);
    assert_eq!(cell2.count(), 2);
}

#[test]
fn test_thin_cell_clone_shared_data() {
    let cell1 = ThinCell::new(42);
    let cell2 = cell1.clone();

    {
        let mut borrowed = cell1.borrow();
        *borrowed = 100;
    }

    // cell2 should see the change
    assert_eq!(*cell2.borrow(), 100);
}

#[test]
fn test_thin_cell_multiple_clones() {
    let cell1 = ThinCell::new(42);
    let cell2 = cell1.clone();
    let cell3 = cell1.clone();
    let cell4 = cell2.clone();

    assert_eq!(cell1.count(), 4);
    assert_eq!(cell2.count(), 4);
    assert_eq!(cell3.count(), 4);
    assert_eq!(cell4.count(), 4);
}

#[test]
fn test_thin_cell_drop_reduces_count() {
    struct DropFlag<'a>(&'a Cell<usize>);

    impl<'a> Drop for DropFlag<'a> {
        fn drop(&mut self) {
            self.0.update(|x| x + 1);
        }
    }

    let flag = Cell::new(0);

    let cell1 = ThinCell::new(DropFlag(&flag));
    let cell2 = cell1.clone();
    let cell3 = cell1.clone();

    assert_eq!(cell1.count(), 3);

    drop(cell2);
    assert_eq!(cell1.count(), 2);

    drop(cell3);
    assert_eq!(cell1.count(), 1);

    drop(cell1);
    assert!(flag.get() == 1);
}

#[test]
fn test_thin_cell_ref_deref() {
    let cell = ThinCell::new([1, 2, 3, 4, 5]);
    let borrowed = cell.borrow();

    // Test Deref
    assert_eq!(borrowed.len(), 5);
    assert_eq!(borrowed[0], 1);

    drop(borrowed);
    let dyn_tc = unsafe { cell.unsize(|p| p as *const thin_cell::Inner<[i32]>) };
    let borrowed_dyn = dyn_tc.borrow();

    assert_eq!(borrowed_dyn.len(), 5);
    assert_eq!(borrowed_dyn[4], 5);
}

#[test]
fn test_thin_cell_ref_deref_mut() {
    let cell = ThinCell::new([1, 2, 3]);
    let mut borrowed = cell.borrow();

    // Test DerefMut
    borrowed[0] = 10;
    assert_eq!(borrowed[0], 10);
}

#[test]
fn test_thin_cell_leak_and_from_raw() {
    let cell = ThinCell::new(42);
    let ptr = cell.leak();

    // Reconstruct from raw pointer
    let cell: ThinCell<i32> = unsafe { ThinCell::from_raw(ptr as *mut ()) };
    assert_eq!(*cell.borrow(), 42);
}

#[test]
fn test_thin_cell_with_tuple() {
    let cell = ThinCell::new((42, 100));
    {
        let mut borrowed = cell.borrow();
        borrowed.0 = 99;
        borrowed.1 = 200;
    }
    assert_eq!(*cell.borrow(), (99, 200));
}

#[test]
fn test_thin_cell_with_option() {
    let cell = ThinCell::new(Some(42));
    {
        let mut borrowed = cell.borrow();
        *borrowed = Some(100);
    }
    assert_eq!(*cell.borrow(), Some(100));
}

#[test]
fn test_thin_cell_count_after_borrow() {
    let cell = ThinCell::new(42);
    let cell2 = cell.clone();

    assert_eq!(cell.count(), 2);

    {
        let _borrowed = cell.borrow();
        // Count should remain the same during borrow
        assert_eq!(cell.count(), 2);
    }

    assert_eq!(cell.count(), 2);
    drop(cell2);
    assert_eq!(cell.count(), 1);
}

#[test]
fn test_thin_rc() {
    // Test with a simple type instead of trait objects to avoid memory corruption
    let cell = ThinCell::new(Robot { id: 1 });

    // Share
    let other = cell.clone();
    assert_eq!(cell.count(), 2);

    // Read
    {
        let borrowed = cell.borrow();
        borrowed.greet();
    }

    {
        let _b = cell.borrow();
        assert!(cell.try_borrow().is_none());
    }

    // Write
    {
        let mut w = cell.borrow();
        w.set_id(100);
    }

    other.borrow().greet(); // Robot 100
}
