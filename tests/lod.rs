use layerprocgen::lod::{LodError, LodTile, MAX_TILES, select_coverage};
fn t(domain: u32, level: u8, x: i32, z: i32) -> LodTile {
    LodTile {
        domain,
        level,
        x,
        z,
    }
}
fn kids(tile: LodTile) -> Vec<LodTile> {
    (0..2)
        .flat_map(|x| {
            (0..2).map(move |z| t(tile.domain, tile.level - 1, tile.x * 2 + x, tile.z * 2 + z))
        })
        .collect()
}
#[test]
fn parent_waits_for_all_four_direct_children() {
    let root = t(0, 2, 0, 0);
    let children = kids(root);
    for count in 0..4 {
        let mut available = vec![root];
        available.extend_from_slice(&children[..count]);
        let result = select_coverage(&available, &[root]).unwrap();
        assert_eq!(result.active, vec![root]);
        assert!(result.missing.is_empty());
    }
    let mut available = vec![root];
    available.extend(&children);
    let result = select_coverage(&available, &[root]).unwrap();
    assert_eq!(result.active, children);
    assert!(result.missing.is_empty());
}
#[test]
fn nested_refinement_and_sparse_missing_branches() {
    let root = t(0, 2, 0, 0);
    let children = kids(root);
    let grandchildren = kids(children[0]);
    let mut available = vec![root];
    available.extend(&children);
    available.extend(&grandchildren);
    let result = select_coverage(&available, &[root]).unwrap();
    let mut expected = children[1..].to_vec();
    expected.extend(&grandchildren);
    expected.sort();
    assert_eq!(result.active, expected);
    assert!(result.missing.is_empty());
    let result = select_coverage(&[grandchildren[0]], &[root]).unwrap();
    assert_eq!(result.active, vec![grandchildren[0]]);
    let mut missing = grandchildren[1..].to_vec();
    missing.extend_from_slice(&children[1..]);
    missing.sort();
    assert_eq!(result.missing, missing);
    let huge_root = t(0, 30, 0, 0);
    assert_eq!(
        select_coverage(&[], &[huge_root]).unwrap().missing,
        vec![huge_root]
    );
}
#[test]
fn negative_coordinates_domains_and_order_are_stable() {
    let root = t(1, 2, -1, -1);
    let children = kids(root);
    let leaf = t(1, 0, -1, -1);
    let other = t(2, 0, -1, -1);
    let a = select_coverage(&[leaf, leaf, other], &[root]).unwrap();
    let b = select_coverage(&[other, leaf], &[root]).unwrap();
    assert_eq!(a, b);
    assert_eq!(a.active, vec![leaf]);
    assert!(a.missing.contains(&children[0]));
    assert!(!a.active.contains(&other));
    let mut ready = kids(root);
    ready.push(root);
    let selected = select_coverage(&ready, &[root]).unwrap();
    assert_eq!(selected.active, kids(root));
    assert!(selected.missing.is_empty());
}
#[test]
fn grandchildren_do_not_replace_a_missing_immediate_child() {
    let root = t(0, 2, 0, 0);
    let children = kids(root);
    let mut ready = vec![root];
    ready.extend_from_slice(&children[1..]);
    ready.extend(kids(children[0]));
    let selection = select_coverage(&ready, &[root]).unwrap();
    assert_eq!(selection.active, vec![root]);
    assert!(selection.missing.is_empty());
}
#[test]
fn invalid_inputs_and_work_are_rejected() {
    let root = t(0, 2, 0, 0);
    assert_eq!(
        select_coverage(&[], &[t(0, 31, 0, 0)]),
        Err(LodError::InvalidLevel)
    );
    assert_eq!(
        select_coverage(&[], &[t(0, 30, 2, 0)]),
        Err(LodError::CoordinateOverflow)
    );
    assert_eq!(
        select_coverage(&[], &[root, kids(root)[0]]),
        Err(LodError::OverlappingRoots)
    );
    assert_eq!(
        select_coverage(&[], &[root, root]),
        Err(LodError::OverlappingRoots)
    );
    assert_eq!(
        select_coverage(&vec![root; MAX_TILES + 1], &[]),
        Err(LodError::InputLimit)
    );
    let mut roots: Vec<_> = (0..MAX_TILES)
        .map(|domain| t(domain as u32, 0, 0, 0))
        .collect();
    let available = roots.clone();
    roots[0].level = 1;
    assert_eq!(
        select_coverage(&available, &roots),
        Err(LodError::WorkLimit)
    );
}
