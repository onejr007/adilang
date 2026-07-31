// ADILang scene model — dunia 3D hasil evaluasi.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).

use crate::ast::{EventKind, Handler};

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MeshKind {
    Sphere,
    Box,
    Torus,
    Icosa,
    Ring,
    Plane,
    Grid,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MaterialKind {
    Solid,
    Wire,
    Glow,
    Points,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LightKind {
    Point,
    Ambient,
}

#[derive(Debug, Clone)]
pub struct MeshParams {
    pub radius: f64,
    pub tube: f64,
    pub inner: f64,
    pub segments: f64,
    pub size: f64,
    pub count: f64,
}

impl Default for MeshParams {
    fn default() -> Self {
        Self { radius: 1.0, tube: 0.05, inner: 1.0, segments: 2.0, size: 10.0, count: 16.0 }
    }
}

#[derive(Debug, Clone)]
pub struct Transform {
    pub pos: [f64; 3],
    pub rot: [f64; 3],
    pub scale: [f64; 3], // per-axis (uniform bila ketiga sama)
}

impl Default for Transform {
    fn default() -> Self {
        Self { pos: [0.0, 0.0, 0.0], rot: [0.0, 0.0, 0.0], scale: [1.0, 1.0, 1.0] }
    }
}

#[derive(Debug, Clone)]
pub struct EntityState {
    pub id: String,
    pub transform: Transform,
    pub color: [f64; 4], // rgba 0..1
    pub material: MaterialKind,
    pub mesh: MeshKind,
    pub mesh_params: MeshParams,
    pub handlers: Vec<Handler>,
}

#[derive(Debug, Clone)]
pub struct CameraState {
    pub id: String,
    pub pos: [f64; 3],
    pub look: [f64; 3],
    pub fov: f64,
}

impl Default for CameraState {
    fn default() -> Self {
        Self { id: "cam".into(), pos: [0.0, 1.6, 7.0], look: [0.0, 0.0, 0.0], fov: 55.0 }
    }
}

#[derive(Debug, Clone)]
pub struct LightState {
    pub id: String,
    pub kind: LightKind,
    pub pos: [f64; 3],
    pub color: [f64; 3],
    pub intensity: f64,
}

#[derive(Debug, Clone)]
pub struct World {
    pub name: String,
    pub camera: CameraState,
    pub lights: Vec<LightState>,
    pub entities: Vec<EntityState>,
    pub frame_handlers: Vec<Handler>,
    pub speak_handlers: Vec<Handler>,
    pub silent_handlers: Vec<Handler>,
    pub click_handlers: Vec<Handler>,
}

impl World {
    pub fn new(name: String) -> Self {
        Self {
            name,
            camera: CameraState::default(),
            lights: vec![LightState {
                id: "default".into(),
                kind: LightKind::Point,
                pos: [5.0, 6.0, 4.0],
                color: [1.0, 0.95, 0.9],
                intensity: 1.5,
            }],
            entities: Vec::new(),
            frame_handlers: Vec::new(),
            speak_handlers: Vec::new(),
            silent_handlers: Vec::new(),
            click_handlers: Vec::new(),
        }
    }

    pub fn entity_mut(&mut self, id: &str) -> Option<&mut EntityState> {
        self.entities.iter_mut().find(|e| e.id == id)
    }

    pub fn entity(&self, id: &str) -> Option<&EntityState> {
        self.entities.iter().find(|e| e.id == id)
    }

    pub fn handlers_for(&self, event: EventKind, entity_id: &str) -> Vec<&Handler> {
        let mut out: Vec<&Handler> = self
            .entities
            .iter()
            .find(|e| e.id == entity_id)
            .map(|e| e.handlers.iter().filter(|h| h.event == event).collect())
            .unwrap_or_default();
        // world-level handlers untuk event tersebut
        let world_handlers: Vec<&Handler> = match event {
            EventKind::Frame => self.frame_handlers.iter().collect(),
            EventKind::Speak => self.speak_handlers.iter().collect(),
            EventKind::Silent => self.silent_handlers.iter().collect(),
            EventKind::Click => self.click_handlers.iter().collect(),
        };
        out.extend(world_handlers);
        out
    }
}
