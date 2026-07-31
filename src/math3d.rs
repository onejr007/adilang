// ADILang math3d — minimal mat4 / vec3 untuk renderer.
// Dirancang & ditulis oleh AI (ADI Agent Ecosystem).

pub const PI: f64 = std::f64::consts::PI;

pub type Mat4 = [f32; 16]; // column-major, GL convention
pub type Vec3 = [f32; 3];

pub fn identity() -> Mat4 {
    [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0]
}

pub fn mul(a: &Mat4, b: &Mat4) -> Mat4 {
    let mut out = [0.0f32; 16];
    for c in 0..4 {
        for r in 0..4 {
            out[c * 4 + r] = a[0 * 4 + r] * b[c * 4 + 0]
                + a[1 * 4 + r] * b[c * 4 + 1]
                + a[2 * 4 + r] * b[c * 4 + 2]
                + a[3 * 4 + r] * b[c * 4 + 3];
        }
    }
    out
}

pub fn translate(x: f32, y: f32, z: f32) -> Mat4 {
    let mut m = identity();
    m[12] = x;
    m[13] = y;
    m[14] = z;
    m
}

pub fn scale(s: f32) -> Mat4 {
    let mut m = identity();
    m[0] = s;
    m[5] = s;
    m[10] = s;
    m
}

pub fn rot_x(a: f32) -> Mat4 {
    let c = a.cos();
    let s = a.sin();
    [
        1.0, 0.0, 0.0, 0.0,
        0.0, c, s, 0.0,
        0.0, -s, c, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

pub fn rot_y(a: f32) -> Mat4 {
    let c = a.cos();
    let s = a.sin();
    [
        c, 0.0, -s, 0.0,
        0.0, 1.0, 0.0, 0.0,
        s, 0.0, c, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

pub fn rot_z(a: f32) -> Mat4 {
    let c = a.cos();
    let s = a.sin();
    [
        c, s, 0.0, 0.0,
        -s, c, 0.0, 0.0,
        0.0, 0.0, 1.0, 0.0,
        0.0, 0.0, 0.0, 1.0,
    ]
}

pub fn perspective(fov_y_deg: f32, aspect: f32, near: f32, far: f32) -> Mat4 {
    let f = 1.0 / (fov_y_deg.to_radians() / 2.0).tan();
    let nf = 1.0 / (near - far);
    [
        f / aspect, 0.0, 0.0, 0.0,
        0.0, f, 0.0, 0.0,
        0.0, 0.0, (far + near) * nf, -1.0,
        0.0, 0.0, 2.0 * far * near * nf, 0.0,
    ]
}

fn dot(a: &Vec3, b: &Vec3) -> f32 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

fn sub(a: &Vec3, b: &Vec3) -> Vec3 {
    [a[0] - b[0], a[1] - b[1], a[2] - b[2]]
}

fn cross(a: &Vec3, b: &Vec3) -> Vec3 {
    [
        a[1] * b[2] - a[2] * b[1],
        a[2] * b[0] - a[0] * b[2],
        a[0] * b[1] - a[1] * b[0],
    ]
}

fn norm(v: &Vec3) -> Vec3 {
    let l = dot(v, v).sqrt();
    if l > 1e-8 {
        [v[0] / l, v[1] / l, v[2] / l]
    } else {
        *v
    }
}

pub fn look_at(eye: &Vec3, target: &Vec3, up: &Vec3) -> Mat4 {
    let f = norm(&sub(target, eye));
    let s = norm(&cross(&f, up));
    let u = cross(&s, &f);
    [
        s[0], u[0], -f[0], 0.0,
        s[1], u[1], -f[1], 0.0,
        s[2], u[2], -f[2], 0.0,
        -dot(&s, eye), -dot(&u, eye), dot(&f, eye), 1.0,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn identity_mul() {
        let a = identity();
        assert_eq!(mul(&a, &a), a);
    }

    #[test]
    fn perspective_finite() {
        let p = perspective(55.0, 1.5, 0.1, 100.0);
        assert!(p.iter().all(|v| v.is_finite()));
    }
}
