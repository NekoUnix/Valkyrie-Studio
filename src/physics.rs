//! Fixed-step spring approximation for authored physics3.json chains.
//! This is app code. The separate Purism Core C library only deforms the mesh.
use crate::purism::Parameter;
use anyhow::{Context, Result, ensure};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Clone, Copy, Default)]
struct Vec2 { x: f32, y: f32 }
impl std::ops::Add for Vec2 { type Output=Self; fn add(self, b: Self)->Self { Self{x:self.x+b.x,y:self.y+b.y} } }
impl std::ops::Sub for Vec2 { type Output=Self; fn sub(self, b: Self)->Self { Self{x:self.x-b.x,y:self.y-b.y} } }
impl std::ops::Mul<f32> for Vec2 { type Output=Self; fn mul(self, b:f32)->Self { Self{x:self.x*b,y:self.y*b} } }

#[derive(Clone, Copy)]
enum Axis { X, Y, Angle }
impl Axis { fn parse(s: &str) -> Option<Self> { match s { "X"=>Some(Self::X), "Y"=>Some(Self::Y), "Angle"=>Some(Self::Angle), _=>None } } }
struct Input { parameter: usize, weight: f32, axis: Axis, reflect: bool }
struct Output { parameter: usize, vertex: usize, axis: Axis, scale: f32, weight: f32, reflect: bool }
struct Particle { rest: Vec2, position: Vec2, previous: Vec2, mobility: f32, delay: f32, acceleration: f32 }
struct Rig {
    inputs: Vec<Input>, outputs: Vec<Output>, particles: Vec<Particle>, accumulator: f32,
    position_norm: [f32; 3], angle_norm: [f32; 3],
}
pub struct Physics { rigs: Vec<Rig> }

fn number(value: &Value, key: &str, default: f32) -> f32 {
    value[key].as_f64().filter(|v| v.is_finite()).map(|v| v as f32).unwrap_or(default)
}
fn norm(value: &Value, default: [f32; 3]) -> [f32; 3] {
    [number(value, "Minimum", default[0]), number(value, "Default", default[1]), number(value, "Maximum", default[2])]
}
fn normalized(value: f32, bounds: [f32; 3], output: [f32; 3]) -> f32 {
    let [low, center, high] = bounds;
    let [out_low, out_center, out_high] = output;
    let value = value.clamp(low, high);
    if value >= center { out_center + if high > center { (value-center)/(high-center)*(out_high-out_center) } else { 0.0 } }
    else { out_center - if center > low { (center-value)/(center-low)*(out_center-out_low) } else { 0.0 } }
}

impl Physics {
    pub fn load(definition: &Value, parameters: &[Parameter]) -> Result<Self> {
        let settings = definition["PhysicsSettings"].as_array().context("PhysicsSettings must be an array")?;
        ensure!(settings.len() <= 256, "Physics setting limit exceeded");
        let ids: HashMap<_, _> = parameters.iter().enumerate().map(|(i, p)| (p.id.as_str(), i)).collect();
        let mut rigs = Vec::new();
        for setting in settings {
            let inputs = setting["Input"].as_array().context("Physics Input must be an array")?;
            let outputs = setting["Output"].as_array().context("Physics Output must be an array")?;
            let vertices = setting["Vertices"].as_array().context("Physics Vertices must be an array")?;
            ensure!(inputs.len() <= 256 && outputs.len() <= 256 && vertices.len() <= 64, "Physics chain limit exceeded");
            let inputs = inputs.iter().filter_map(|item| {
                let id = item["Source"]["Id"].as_str()?;
                Some(Input { parameter: *ids.get(id)?, axis: Axis::parse(item["Type"].as_str()?)?,
                    weight: number(item, "Weight", 0.0) * 0.01, reflect: item["Reflect"] == true })
            }).collect();
            let outputs = outputs.iter().filter_map(|item| {
                let id = item["Destination"]["Id"].as_str()?;
                let vertex = item["VertexIndex"].as_u64()? as usize;
                if vertex == 0 || vertex >= vertices.len() { return None }
                Some(Output { parameter: *ids.get(id)?, vertex,
                    axis: Axis::parse(item["Type"].as_str()?)?, scale: number(item, "Scale", 0.0),
                    weight: number(item, "Weight", 0.0) * 0.01, reflect: item["Reflect"] == true })
            }).collect();
            let particles = vertices.iter().map(|item| {
                let point = Vec2 { x: number(&item["Position"], "X", 0.0), y: number(&item["Position"], "Y", 0.0) };
                Particle { rest: point, position: point, previous: point,
                    mobility: number(item, "Mobility", 0.0), delay: number(item, "Delay", 0.0),
                    acceleration: number(item, "Acceleration", 0.0) }
            }).collect::<Vec<_>>();
            if particles.len() > 1 {
                rigs.push(Rig { inputs, outputs, particles, accumulator: 0.0,
                    position_norm: norm(&setting["Normalization"]["Position"], [-10.0,0.0,10.0]),
                    angle_norm: norm(&setting["Normalization"]["Angle"], [-10.0,0.0,10.0]) });
            }
        }
        Ok(Self { rigs })
    }
    pub fn rig_count(&self) -> usize { self.rigs.len() }
    pub fn step(&mut self, parameters: &mut [Parameter], dt: f32) {
        if !(dt > 0.0 && dt.is_finite()) { return }
        const STEP: f32 = 1.0/60.0;
        for rig in &mut self.rigs {
            let mut x=0.0; let mut y=0.0; let mut angle=0.0;
            for input in &rig.inputs {
                let p = &parameters[input.parameter];
                let output = if matches!(input.axis, Axis::Angle) { rig.angle_norm } else { rig.position_norm };
                let center = output[1];
                let n = (normalized(p.value, [p.min,p.default,p.max], output)-center)
                    *input.weight*if input.reflect {-1.0} else {1.0};
                match input.axis { Axis::X=>x+=n, Axis::Y=>y+=n, Axis::Angle=>angle+=n }
            }
            rig.accumulator=(rig.accumulator+dt).min(STEP*8.0);
            let (s,c)=(angle.to_radians()).sin_cos();
            while rig.accumulator+0.000001>=STEP {
                rig.accumulator-=STEP;
                rig.particles[0].previous=rig.particles[0].position;
                rig.particles[0].position=rig.particles[0].rest+Vec2{x,y};
                for j in 1..rig.particles.len() {
                    let parent=rig.particles[j-1].position;
                    let rest=rig.particles[j].rest-rig.particles[j-1].rest;
                    let p=&mut rig.particles[j];
                    let before=p.position;
                    let rotated=Vec2{x:rest.x*c-rest.y*s,y:rest.x*s+rest.y*c};
                    let target=parent+rotated;
                    let damping=p.mobility.clamp(0.0,0.999).powf(STEP*60.0);
                    let velocity=(p.position-p.previous)*damping;
                    let response=p.acceleration.max(0.01)/(1.0+p.delay.max(0.0));
                    let stiffness=(1.0-(-response*STEP*60.0).exp()).clamp(0.01,0.8);
                    p.position=p.position+velocity+(target-p.position)*stiffness;
                    let delta=p.position-parent;
                    let distance=delta.x.hypot(delta.y);
                    let radius=rest.x.hypot(rest.y);
                    if distance>0.0001 && radius>0.0 { p.position=parent+delta*(radius/distance); }
                    if !(p.position.x.is_finite() && p.position.y.is_finite()) { p.position=target; }
                    p.previous=before;
                }
            }
            for output in &rig.outputs {
                let particle=&rig.particles[output.vertex];
                let parent=&rig.particles[output.vertex-1];
                let delta=particle.position-parent.position;
                let rest=particle.rest-parent.rest;
                let measure=match output.axis {
                    Axis::X=>delta.x-rest.x, Axis::Y=>delta.y-rest.y,
                    Axis::Angle=>(rest.y*delta.x-rest.x*delta.y).atan2(rest.x*delta.x+rest.y*delta.y),
                };
                let raw=measure*output.scale*output.weight*0.25*if output.reflect {1.0} else {-1.0};
                let p=&mut parameters[output.parameter];
                let range=(p.max-p.default).max(p.default-p.min);
                let limit=(range*0.8).max(0.001);
                p.value=(p.value+limit*(raw/limit).tanh()).clamp(p.min,p.max);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn authored_chain_is_bounded_under_large_frame_delay() {
        let source = serde_json::json!({"PhysicsSettings":[{"Normalization":{"Position":{"Minimum":-10,"Default":0,"Maximum":10}},
            "Input":[{"Source":{"Id":"Head"},"Type":"X","Weight":100}],
            "Output":[{"Destination":{"Id":"Hair"},"Type":"X","VertexIndex":1,"Scale":10,"Weight":100}],
            "Vertices":[{"Position":{"X":0,"Y":0}},{"Position":{"X":0,"Y":1},"Mobility":0.5,"Acceleration":1}]}]});
        let mut parameters=vec![Parameter{id:"Head".into(),min:-30.0,max:30.0,default:0.0,value:30.0},
            Parameter{id:"Hair".into(),min:-1.0,max:1.0,default:0.0,value:0.0}];
        let mut physics=Physics::load(&source,&parameters).unwrap();
        physics.step(&mut parameters,2.0);
        assert!(parameters[1].value.is_finite() && parameters[1].value.abs()<=1.0);
    }
}
