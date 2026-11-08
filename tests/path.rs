use layerprocgen::path::*;
fn p(x: f64, z: f64) -> Point2 {
    Point2 { x, z }
}
fn flat(_: Point2) -> Option<TerrainSample> {
    Some(TerrainSample {
        height: 0.,
        cost: 0.,
        passable: true,
    })
}
fn options() -> PathOptions {
    PathOptions {
        width: PathWidth {
            surface: 0.2,
            shoulder: 0.,
            material: 0.,
        },
        ..Default::default()
    }
}
#[test]
fn exact_endpoints_heights_and_repeat() {
    let sample = |p: Point2| {
        Some(TerrainSample {
            height: p.x * 0.1,
            ..flat(p)?
        })
    };
    let a = plan_path(sample, p(0., 0.), p(7.3, 1.1), options()).unwrap();
    assert_eq!(
        a,
        plan_path(sample, p(0., 0.), p(7.3, 1.1), options()).unwrap()
    );
    assert_eq!(a.points.first().unwrap().position, p(0., 0.));
    assert_eq!(a.points.last().unwrap().position, p(7.3, 1.1));
    for q in a.points {
        assert_eq!(q.height, q.position.x * 0.1);
    }
}
#[test]
fn cannot_jump_over_water() {
    let o = PathOptions {
        corridor: 0.,
        ..options()
    };
    let a = plan_path(
        |p| if p.x > 2. && p.x < 3. { None } else { flat(p) },
        p(0., 0.),
        p(6., 0.),
        o,
    );
    assert_eq!(a.unwrap_err(), PathError::NoPath);
}
#[test]
fn whole_width_is_checked() {
    let o = PathOptions {
        corridor: 0.,
        ..Default::default()
    };
    let a = plan_path(
        |p| if p.z.abs() > 0.6 { None } else { flat(p) },
        p(0., 0.),
        p(4., 0.),
        o,
    );
    assert_eq!(a.unwrap_err(), PathError::NoPath);
}
#[test]
fn diagonal_zero_corridor_is_legal() {
    let o = PathOptions {
        corridor: 0.,
        ..options()
    };
    assert!(plan_path(flat, p(0., 0.), p(4., 4.), o).is_ok());
}
#[test]
fn exact_grade_limit_is_legal() {
    let o = PathOptions {
        corridor: 0.,
        ..options()
    };
    assert!(
        plan_path(
            |p| Some(TerrainSample {
                height: 0.6 * p.x,
                ..flat(p)?
            }),
            p(0., 0.),
            p(4., 0.),
            o
        )
        .is_ok()
    );
}
#[test]
fn over_grade_is_blocked() {
    let o = PathOptions {
        corridor: 0.,
        ..options()
    };
    assert_eq!(
        plan_path(
            |p| Some(TerrainSample {
                height: 0.61 * p.x,
                ..flat(p)?
            }),
            p(0., 0.),
            p(4., 0.),
            o
        )
        .unwrap_err(),
        PathError::NoPath
    );
}
#[test]
fn cache_aliasing_cannot_bypass_work_budget() {
    let o = PathOptions {
        sample_spacing: 1e-5,
        ..Default::default()
    };
    assert_eq!(
        plan_path(flat, p(1e16, 1e16), p(1e16 + 2., 1e16), o).unwrap_err(),
        PathError::SampleBudget
    );
}
#[test]
fn changed_snapshot_is_not_cached() {
    let a = plan_path(flat, p(0., 0.), p(8.1, 0.), options()).unwrap();
    assert!(!a.points.is_empty());
    assert_eq!(
        plan_path(|_| None, p(0., 0.), p(8.1, 0.), options()).unwrap_err(),
        PathError::NoPath
    );
}
#[test]
fn invalid_samples_and_options_fail() {
    assert_eq!(
        plan_path(
            |_| Some(TerrainSample {
                height: f64::NAN,
                cost: 0.,
                passable: true
            }),
            p(0., 0.),
            p(4., 0.),
            options()
        )
        .unwrap_err(),
        PathError::InvalidTerrain
    );
    let o = PathOptions {
        grid_spacing: 0.,
        ..options()
    };
    assert_eq!(
        plan_path(flat, p(0., 0.), p(1., 0.), o).unwrap_err(),
        PathError::InvalidInput
    );
    let o = PathOptions {
        max_samples: 1,
        ..options()
    };
    assert_eq!(
        plan_path(flat, p(0., 0.), p(1., 0.), o).unwrap_err(),
        PathError::SampleBudget
    );
}
#[test]
fn cost_detour_survives_smoothing() {
    let a = plan_path(
        |p| {
            Some(TerrainSample {
                cost: if p.x > 3. && p.x < 9. && p.z.abs() < 1. {
                    100.
                } else {
                    0.
                },
                ..flat(p)?
            })
        },
        p(0., 0.),
        p(12., 0.),
        options(),
    )
    .unwrap();
    assert!(a.raw.iter().any(|q| q.position.z.abs() >= 1.));
    assert!(a.points.iter().any(|q| q.position.z.abs() >= 1.));
}

#[test]
fn smoothed_detour_preserves_clearance() {
    let o = PathOptions {
        sample_spacing: 0.25,
        ..options()
    };
    let blocked = |q: Point2| (q.x - 4.).hypot(q.z) < 1.5;
    let a = plan_path(
        |q| if blocked(q) { None } else { flat(q) },
        p(0., 0.),
        p(8., 0.),
        o,
    )
    .unwrap();
    assert!(a.points.iter().any(|q| q.position.z.abs() > 1.5));
    // Inspect the returned chords and both surface edges more densely than planning.
    for pair in a.points.windows(2) {
        let a = pair[0].position;
        let b = pair[1].position;
        let d = (b.x - a.x).hypot(b.z - a.z);
        assert!(d <= o.sample_spacing + 1e-9);
        for i in 0..=10 {
            for offset in [-o.width.surface / 2., 0., o.width.surface / 2.] {
                let q = p(
                    a.x + (b.x - a.x) * i as f64 / 10. - (b.z - a.z) / d * offset,
                    a.z + (b.z - a.z) * i as f64 / 10. + (b.x - a.x) / d * offset,
                );
                assert!(!blocked(q), "smoothed path clips blocked terrain at {q:?}");
            }
        }
    }
}
