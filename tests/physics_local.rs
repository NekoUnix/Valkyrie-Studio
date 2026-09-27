//! Opt-in diagnostic for a locally owned model. No model assets are included.
use std::{env, path::Path};
use valkyrie_studio::{
    model::ModelAssets,
    physics::{GroupSettings, Physics, PhysicsSettings},
    purism::CubismModel,
};

#[test]
#[ignore = "set VALKYRIE_TEST_MODEL to a local .model3.json"]
fn authored_physics_response() {
    let path = env::var("VALKYRIE_TEST_MODEL").expect("set VALKYRIE_TEST_MODEL");
    let assets = ModelAssets::open(Path::new(&path)).unwrap();
    let mut model = CubismModel::load(Path::new(""), &assets.moc, assets.textures.len()).unwrap();
    let defaults = model.parameters().to_vec();
    let mut physics =
        Physics::load(assets.physics.as_ref().expect("physics3.json"), &defaults).unwrap();
    let mut settings = PhysicsSettings::default();
    for group in physics.groups() {
        if group.name.to_ascii_lowercase().contains("tail") {
            settings.groups.insert(
                group.id,
                GroupSettings {
                    inertia: 0.55,
                    ..Default::default()
                },
            );
        }
    }
    physics.configure(&settings);
    println!(
        "{} authored rigs, {} parameters",
        physics.rig_count(),
        defaults.len()
    );

    let mut released_trace = Vec::new();
    for (label, angle) in [("neutral", 0.0), ("head right", 20.0), ("released", 0.0)] {
        for frame in 0..120 {
            for (parameter, original) in model.parameters_mut().iter_mut().zip(&defaults) {
                parameter.value = original.default;
                if parameter.id == "ParamAngleX" {
                    parameter.value = angle;
                }
            }
            physics.step(model.parameters_mut(), 1.0 / 60.0);
            if label == "released" {
                released_trace.push(
                    model
                        .parameters()
                        .iter()
                        .find(|p| p.id == "Param_Angle_Rotation25")
                        .unwrap()
                        .value,
                );
            }
            if frame % 30 == 29 {
                let sample = model
                    .parameters()
                    .iter()
                    .find(|p| p.id == "Param72")
                    .unwrap();
                println!("{label} frame {} Param72 {:.3}", frame + 1, sample.value);
            }
        }
        let mut changed = model
            .parameters()
            .iter()
            .zip(&defaults)
            .filter_map(|(p, original)| {
                let delta = p.value - original.default;
                (delta.abs() > 0.01).then_some((p.id.as_str(), delta, p.min, p.max))
            })
            .collect::<Vec<_>>();
        changed.sort_by(|a, b| b.1.abs().total_cmp(&a.1.abs()));
        if label == "released" {
            let last = &released_trace[90..];
            let span = last.iter().copied().fold(f32::NEG_INFINITY, f32::max)
                - last.iter().copied().fold(f32::INFINITY, f32::min);
            println!("tail last half-second span: {span:.3}");
            assert!(
                span < 0.5,
                "tail motion should stop oscillating after release"
            );
        }
        println!("{label}: {} changed outputs", changed.len());
        for (id, delta, min, max) in changed.into_iter().take(20) {
            println!("  {id:32} {delta:>8.3} range [{min:.1}, {max:.1}]");
        }
    }
    physics.reset();
    let mut tail_trace = Vec::new();
    for frame in 0..360 {
        for (parameter, original) in model.parameters_mut().iter_mut().zip(&defaults) {
            parameter.value = original.default;
            if parameter.id == "Param111" && (30..90).contains(&frame) {
                parameter.value = original.default + (original.max - original.default) * 0.65;
            }
        }
        physics.step(model.parameters_mut(), 1.0 / 60.0);
        tail_trace.push(
            model
                .parameters()
                .iter()
                .find(|p| p.id == "Param_Angle_Rotation24")
                .unwrap()
                .value,
        );
    }
    let peak = tail_trace[30..150]
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max)
        - tail_trace[30..150]
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min);
    let settled = tail_trace[300..]
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max)
        - tail_trace[300..]
            .iter()
            .copied()
            .fold(f32::INFINITY, f32::min);
    println!("direct tail drive: peak span {peak:.3}, late span {settled:.3}");
    assert!(peak > 0.5, "direct tail drive should move the rig");
    assert!(
        settled < 0.5,
        "direct tail drive should settle without jitter"
    );
}
