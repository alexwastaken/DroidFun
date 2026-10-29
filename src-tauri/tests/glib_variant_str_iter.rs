#![cfg(target_os = "linux")]

use glib::variant::ToVariant;

// RUSTSEC-2024-0429: each of these methods reads the C out-pointer in impl_get.
// Run with --release so optimization cannot hide the original UB.
#[test]
fn variant_string_iterator_reads_all_affected_paths() {
    let expected = ["Nimbus", "", "Boo 🦇", "Meep", "Claw"];
    let variant = expected.to_variant();

    assert_eq!(
        variant.array_iter_str().unwrap().collect::<Vec<_>>(),
        expected
    );
    assert_eq!(
        variant.array_iter_str().unwrap().rev().collect::<Vec<_>>(),
        expected.into_iter().rev().collect::<Vec<_>>()
    );

    let mut forward = variant.array_iter_str().unwrap();
    assert_eq!(forward.next(), Some("Nimbus"));
    assert_eq!(forward.nth(1), Some("Boo 🦇"));
    assert_eq!(forward.last(), Some("Claw"));

    let mut backward = variant.array_iter_str().unwrap();
    assert_eq!(backward.next_back(), Some("Claw"));
    assert_eq!(backward.nth_back(1), Some("Boo 🦇"));
    assert_eq!(backward.next(), Some("Nimbus"));
    assert_eq!(backward.next_back(), Some(""));
    assert_eq!(backward.next(), None);
    assert_eq!(backward.next_back(), None);
    assert_eq!(backward.nth(0), None);
    assert_eq!(backward.nth_back(0), None);
    assert_eq!(backward.last(), None);
}

#[test]
fn variant_string_iterator_handles_empty_and_out_of_range_access() {
    let empty = Vec::<String>::new().to_variant();
    assert_eq!(empty.array_iter_str().unwrap().next(), None);
    assert_eq!(empty.array_iter_str().unwrap().next_back(), None);
    assert_eq!(empty.array_iter_str().unwrap().nth(0), None);
    assert_eq!(empty.array_iter_str().unwrap().nth_back(0), None);
    assert_eq!(empty.array_iter_str().unwrap().last(), None);

    let single = ["Claw"].to_variant();
    assert_eq!(single.array_iter_str().unwrap().last(), Some("Claw"));
    assert_eq!(single.array_iter_str().unwrap().nth(usize::MAX), None);
    assert_eq!(single.array_iter_str().unwrap().nth_back(usize::MAX), None);
}
