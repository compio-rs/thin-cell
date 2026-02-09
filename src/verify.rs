use vstd::{layout::size_of, prelude::*};

verus! {

spec const RC_MASK: usize = !0b1;

spec const RC_UNIT: usize = 0b10;

spec const RC_MAX: usize = RC_MASK;

spec const RC_MAX_COUNT: usize = RC_MAX >> 1;

spec const BORROW_MASK: usize = 0b1;

spec fn count(state: usize) -> usize {
    (state & RC_MASK) >> 1
}

spec fn is_shared(state: usize) -> bool {
    count(state) > 1
}

spec fn is_borrowed(state: usize) -> bool {
    (state % 2) == 1
}

spec fn is_max(state: usize) -> bool {
    state & RC_MASK == RC_MAX
}

spec fn inc(state: usize) -> Option<usize> {
    if is_max(state) {
        None
    } else {
        Some((state + RC_UNIT) as usize)
    }
}

spec fn dec(state: usize) -> usize {
    (state - RC_UNIT) as usize
}

spec fn borrow(state: usize) -> Option<usize> {
    if is_borrowed(state) {
        None
    } else {
        Some((state + 1) as usize)
    }
}

spec fn unborrow(state: usize) -> usize {
    (state & RC_MASK) as usize
}

proof fn unborrow_unset_mask(state: usize)
    by (bit_vector)
    requires
        is_borrowed(state),
    ensures
        (unborrow(state) & BORROW_MASK) == 0,
{
}

proof fn unborrow_minus_one(state: usize)
    by (bit_vector)
    requires
        is_borrowed(state),
    ensures
        unborrow(state) == state - 1,
{
}

proof fn not_borrowed_no_overflow(state: usize)
    requires
        !is_borrowed(state),
    ensures
        (state + 1) as usize > state,
{
}

proof fn borrow_plus_one(state: usize)
    requires
        !is_borrowed(state),
    ensures
        borrow(state)->0 == state + 1,
{
}

proof fn count_eq(a: usize, b: usize)
    requires
        a == b + 1,
        !is_borrowed(b),
    ensures
        (a & RC_MASK) >> 1 == (b & RC_MASK) >> 1,
{
    assert(count(a) == count(b)) by (bit_vector)
        requires
            a == b + 1,
            !is_borrowed(b),
            count(a) == (a & RC_MASK) >> 1,
            count(b) == (b & RC_MASK) >> 1,
    ;
}

proof fn borrow_perserves_count(state: usize)
    requires
        !is_borrowed(state),
    ensures
        borrow(state) matches Some(new_state) && count(new_state) == count(state),
{
    count_eq(borrow(state)->0, state)
}

proof fn unborrow_preserves_count(state: usize)
    requires
        is_borrowed(state),
    ensures
        count(unborrow(state)) == count(state),
{
    let new = unborrow(state);
    unborrow_minus_one(state);
    count_eq(state, new)
}

proof fn inc_inc(state: usize) -> (ret: usize)
    requires
        !is_max(state),
    ensures
        ret == inc(state)->0,
        ret == state + RC_UNIT,
{
    let v = inc(state)->0;
    assert(state + RC_UNIT <= !0usize) by (bit_vector)
        requires
            !is_max(state),
    ;
    v
}

proof fn count_monotonic(a: usize, b: usize)
    by (bit_vector)
    requires
        a + 1 < b,
    ensures
        count(a) < count(b),
{
}

proof fn inc_count(state: usize)
    requires
        !is_max(state),
    ensures
        count(inc(state)->0) == count(state) + 1,
{
    let v = inc_inc(state);
    assert(count(v) == count(state) + 1) by (bit_vector)
        requires
            v == state + RC_UNIT,
    ;
}

proof fn inc_dec_unchange(state: usize)
    requires
        !is_max(state),
    ensures
        dec(inc(state)->0) == state,
{
    let v = inc_inc(state);
    assert(dec(v) == state) by (bit_vector)
        requires
            v == state + RC_UNIT,
    ;
}

proof fn inc_to_shared(state: usize)
    requires
        !is_max(state),
        count(state) == 1,
    ensures
        is_shared(inc(state)->0),
{
    inc_count(state);
}

#[repr(C)]
struct FatPtr {
    pub ptr: *mut (),
    pub metadata: usize,
}

global layout FatPtr is size == 16, align == 8;

} // verus!
