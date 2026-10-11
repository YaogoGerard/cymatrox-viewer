//! Analytic CymaScope glyph — the Faraday mode a circular dish selects.
//!
//! A real CymaScope imprints sound on a shallow water layer: the surface is
//! driven vertically and, above a threshold, a stationary Faraday pattern locks
//! in — the famous Chladni-like figure of concentric rings and radial petals.
//!
//! A thin-film PDE on a coarse grid cannot *reliably* pick that single figure:
//! for this dish two nearly degenerate modes (e.g. `(9, 1)` and `(6, 2)` at
//! ~30 Hz) grow together and beat, so the picture keeps morphing instead of
//! settling. In the weakly-driven limit the dish instead selects the mode whose
//! natural frequency is closest to the Faraday subharmonic `f/2` — the most
//! resonant mode, hence the one with the highest growth rate and the only one
//! to survive. This module evaluates that mode directly.
//!
//! Mode shape on a dish with a pinned contact line (nodes at the wall):
//!
//! ```text
//!     h(r, θ) = J_m(k_mn · r) · cos(m θ),      k_mn = j_{m,n} / R
//! ```
//!
//! where `j_{m,n}` is the n-th positive zero of the Bessel function `J_m` and
//! `R` is the dish radius (m). The natural angular frequency follows the
//! capillary–gravity dispersion (`depth` is the liquid depth):
//!
//! ```text
//!     ω²_mn = (g · k + (σ/ρ) · k³) · tanh(k · depth)
//! ```
//!
//! The output is a plain height field in metres; rendering is left to the
//! caller. This is the viewer's *display* model of the dish (the crate's fluid
//! solver is a separate, general-purpose simulation).

use std::sync::OnceLock;

use cymatrox::fluid::LiquidSpec;

/// Peak surface elevation (m) the rasterised glyph is normalised to — a
/// physically plausible sub-millimetre Faraday amplitude.
pub const GLYPH_AMPLITUDE_M: f32 = 1.2e-3;

/// Highest azimuthal order and radial order considered when selecting a mode.
const MAX_M: u32 = 16;
const N_MAX: usize = 80;

/// The dish standing mode selected for a tone: `m` nodal diameters and radial
/// wavenumber `k` (1/m), with `k · R` equal to a Bessel zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FaradayMode {
    /// Azimuthal order — the number of nodal diameters.
    pub m: u32,
    /// Radial wavenumber (1/m). `k · dish_radius` is a Bessel zero.
    pub k: f64,
    /// Natural frequency of the mode (Hz).
    pub frequency_hz: f64,
}

/// Selects the dish mode whose natural frequency is closest to the Faraday
/// subharmonic `frequency_hz / 2`.
///
/// * `frequency_hz` — drive (tone) frequency.
/// * `dish_radius_m` — radius `R` of the circular dish.
/// * `liquid` — density, surface tension, depth and gravity of the medium.
pub fn dominant_mode(frequency_hz: f32, dish_radius_m: f32, liquid: &LiquidSpec) -> FaradayMode {
    let omega = std::f64::consts::PI * (frequency_hz as f64).max(0.0);
    let sigma_over_rho = if liquid.density > 0.0 {
        (liquid.surface_tension / liquid.density) as f64
    } else {
        0.0
    };
    let depth = liquid.depth as f64;
    let gravity = liquid.gravity as f64;
    let radius = (dish_radius_m as f64).max(1e-9);

    let zs = zeros();
    let mut best = FaradayMode {
        m: 0,
        k: zs[0][0] / radius,
        frequency_hz: 0.0,
    };
    let mut best_err = f64::INFINITY;
    for (m, order) in zs.iter().enumerate() {
        for &z in order {
            let k = z / radius;
            let omega2 = (gravity * k + sigma_over_rho * k * k * k) * (k * depth).tanh();
            let omega_mn = omega2.max(0.0).sqrt();
            let err = (omega_mn - omega).abs();
            if err < best_err {
                best_err = err;
                best = FaradayMode {
                    m: m as u32,
                    k,
                    frequency_hz: omega_mn / (2.0 * std::f64::consts::PI),
                };
            }
        }
    }
    best
}

/// Rasterises the dominant Faraday glyph for the given tone into `out`.
///
/// `out` is treated as a row-major `side × side` square, `side = sqrt(len)`
/// (`len` must be a perfect square). The dish is the unit disc; cells outside
/// it are zeroed. The field is peak-normalised to [`GLYPH_AMPLITUDE_M`]
/// (metres). Returns the selected [`FaradayMode`].
pub fn rasterise_glyph(
    frequency_hz: f32,
    dish_radius_m: f32,
    liquid: &LiquidSpec,
    out: &mut [f32],
) -> FaradayMode {
    let grid = (out.len() as f64).sqrt().round() as usize;
    if grid == 0 || grid * grid != out.len() {
        return dominant_mode(frequency_hz, dish_radius_m, liquid);
    }
    let mode = dominant_mode(frequency_hz, dish_radius_m, liquid);
    let mf = mode.m as f64;
    let denom = (grid - 1).max(1) as f64;
    let mut max_abs = 0.0f64;
    for gy in 0..grid {
        let py = gy as f64 / denom * 2.0 - 1.0;
        for gx in 0..grid {
            let px = gx as f64 / denom * 2.0 - 1.0;
            let r = (px * px + py * py).sqrt();
            let v = if r <= 1.0 {
                let theta = py.atan2(px);
                bessel_j(mode.m, mode.k * dish_radius_m as f64 * r) * (mf * theta).cos()
            } else {
                0.0
            };
            out[gy * grid + gx] = v as f32;
            if v.abs() > max_abs {
                max_abs = v.abs();
            }
        }
    }
    let scale = if max_abs > 1e-12 {
        GLYPH_AMPLITUDE_M as f64 / max_abs
    } else {
        0.0
    };
    for v in out.iter_mut() {
        *v = (*v as f64 * scale) as f32;
    }
    mode
}

/// Bessel function of the first kind, integer order `m`, argument `x`.
///
/// Numerical Recipes' rational/asymptotic `J0`/`J1` with the stable upward
/// recurrence (for `x > m`) or Miller's downward recurrence (otherwise).
/// Accurate to ~1e-8 — ample for a visual glyph and for locating the zeros.
fn bessel_j(m: u32, x: f64) -> f64 {
    match m {
        0 => bessj0(x),
        1 => bessj1(x),
        _ => bessj_n(m, x),
    }
}

fn bessj_n(n: u32, x: f64) -> f64 {
    if x == 0.0 {
        return 0.0;
    }
    let ax = x.abs();
    let tox = 2.0 / ax;
    let ans = if x > n as f64 {
        // Upward recurrence is stable once x exceeds the order.
        let mut bjm = bessj0(ax);
        let mut bj = bessj1(ax);
        for j in 1..n {
            let bjp = j as f64 * tox * bj - bjm;
            bjm = bj;
            bj = bjp;
        }
        bj
    } else {
        // Miller's downward recurrence, renormalised by the alternating series.
        const ACC: f64 = 40.0;
        const BIGNO: f64 = 1.0e10;
        const BIGNI: f64 = 1.0e-10;
        let mstart = 2 * ((n as usize + (ACC * n as f64).sqrt() as usize) / 2);
        let mut jsum = false;
        let mut bjp = 0.0f64;
        let mut sum = 0.0f64;
        let mut bj = 1.0f64;
        let mut ans = 0.0f64;
        for j in (1..=mstart).rev() {
            let bjm = j as f64 * tox * bj - bjp;
            bjp = bj;
            bj = bjm;
            if bj.abs() > BIGNO {
                bj *= BIGNI;
                bjp *= BIGNI;
                ans *= BIGNI;
                sum *= BIGNI;
            }
            if jsum {
                sum += bj;
            }
            jsum = !jsum;
            if j == n as usize {
                ans = bjp;
            }
        }
        sum = 2.0 * sum - bj;
        ans / sum
    };
    if x < 0.0 && n % 2 == 1 {
        -ans
    } else {
        ans
    }
}

// Rational polynomial coefficients (Numerical Recipes); the leading digits
// happen to approximate well-known constants, which is irrelevant here.
#[allow(clippy::approx_constant)]
fn bessj0(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 8.0 {
        let y = x * x;
        let ans1 = 57568490574.0
            + y * (-13362590354.0
                + y * (651619640.7 + y * (-11214424.18 + y * (77392.33017 + y * (-184.9052456)))));
        let ans2 = 57568490411.0
            + y * (1029532985.0 + y * (9494680.718 + y * (59272.64853 + y * (267.8532712 + y))));
        ans1 / ans2
    } else {
        let z = 8.0 / ax;
        let y = z * z;
        let xx = ax - 0.785398164;
        let ans1 = 1.0
            + y * (-0.1098628627e-2
                + y * (0.2734510407e-4 + y * (-0.2073370639e-5 + y * 0.2093887211e-6)));
        let ans2 = -0.1562499995e-1
            + y * (0.1430488765e-3
                + y * (-0.6911147651e-5 + y * (0.7621095161e-6 + y * (-0.934935152e-7))));
        (0.636619772 / ax).sqrt() * (xx.cos() * ans1 - z * xx.sin() * ans2)
    }
}

#[allow(clippy::approx_constant)]
fn bessj1(x: f64) -> f64 {
    let ax = x.abs();
    if ax < 8.0 {
        let y = x * x;
        let ans1 = x
            * (72362614232.0
                + y * (-7895059235.0
                    + y * (242396853.1
                        + y * (-2972611.439 + y * (15704.48260 + y * (-30.16036606))))));
        let ans2 = 144725228442.0
            + y * (2300535178.0 + y * (18583304.74 + y * (99447.43394 + y * (376.9991397 + y))));
        ans1 / ans2
    } else {
        let z = 8.0 / ax;
        let y = z * z;
        let xx = ax - 2.356194491;
        let ans1 = 1.0
            + y * (0.183105e-2
                + y * (-0.3516396496e-4 + y * (0.2457520174e-5 + y * (-0.240337019e-6))));
        let ans2 = 0.04687499995
            + y * (-0.2002690873e-3
                + y * (0.8449199096e-5 + y * (-0.88228987e-6 + y * 0.105787412e-6)));
        let ans = (0.636619772 / ax).sqrt() * (xx.cos() * ans1 - z * xx.sin() * ans2);
        if x < 0.0 {
            -ans
        } else {
            ans
        }
    }
}

/// The Bessel zero table, computed once: `ZEROS[m][n-1] = j_{m,n}`, for
/// `m ∈ [0, MAX_M]` and the first `N_MAX` zeros of each order.
fn zeros() -> &'static Vec<Vec<f64>> {
    static ZEROS: OnceLock<Vec<Vec<f64>>> = OnceLock::new();
    ZEROS.get_or_init(|| (0..=MAX_M).map(|m| bessel_zeros(m, N_MAX)).collect())
}

fn bessel_zeros(m: u32, n_max: usize) -> Vec<f64> {
    const XMAX: f64 = 320.0;
    const STEP: f64 = 0.2;
    let mut out = Vec::with_capacity(n_max);
    let mut xa = 1.0e-3;
    let mut fa = bessel_j(m, xa);
    while xa < XMAX && out.len() < n_max {
        let xb = xa + STEP;
        let fb = bessel_j(m, xb);
        if fa == 0.0 {
            out.push(xa);
        } else if fb != 0.0 && fa.signum() != fb.signum() {
            let (mut lo, mut hi) = (xa, xb);
            let mut flo = fa;
            for _ in 0..80 {
                let mid = 0.5 * (lo + hi);
                let fm = bessel_j(m, mid);
                if fm == 0.0 || hi - lo < 1e-12 {
                    lo = mid;
                    hi = mid;
                    break;
                }
                if flo.signum() != fm.signum() {
                    hi = mid;
                } else {
                    lo = mid;
                    flo = fm;
                }
            }
            out.push(0.5 * (lo + hi));
        }
        fa = fb;
        xa = xb;
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn water() -> LiquidSpec {
        LiquidSpec {
            density: 1000.0,
            surface_tension: 0.072,
            depth: 0.004,
            damping: 0.2,
            gravity: 9.81,
        }
    }

    #[test]
    fn bessel_matches_known_zeros() {
        // First zeros of J0, J1, J2 (Abramowitz & Stegun).
        let z0 = bessel_zeros(0, 3);
        assert!((z0[0] - 2.404826).abs() < 1e-5, "{z0:?}");
        assert!((z0[1] - 5.520078).abs() < 1e-5, "{z0:?}");
        let z1 = bessel_zeros(1, 2);
        assert!((z1[0] - 3.831706).abs() < 1e-5, "{z1:?}");
        assert!((z1[1] - 7.015587).abs() < 1e-5, "{z1:?}");
        let z2 = bessel_zeros(2, 2);
        assert!((z2[0] - 5.135622).abs() < 1e-5, "{z2:?}");
        assert!((z2[1] - 8.417244).abs() < 1e-5, "{z2:?}");
    }

    #[test]
    fn mode_fineness_rises_monotonically_with_tone() {
        let l = water();
        let band = [20.0, 40.0, 60.0, 90.0, 150.0, 250.0, 400.0, 700.0, 1000.0];
        let mut prev_k = 0.0;
        for &f in &band {
            let m = dominant_mode(f, 0.025, &l);
            assert!(m.k > prev_k, "k not rising at {f} Hz");
            prev_k = m.k;
        }
    }

    #[test]
    fn glyph_is_bounded_and_flat_outside_the_pool() {
        let n = 96;
        let mut f = vec![0.0f32; n * n];
        rasterise_glyph(60.0, 0.025, &water(), &mut f);
        assert!(f.iter().all(|v| v.is_finite()));
        let peak = f.iter().fold(0.0f32, |a, &b| a.max(b.abs()));
        assert!((peak - GLYPH_AMPLITUDE_M).abs() < 1e-5, "peak={peak}");
        assert_eq!(f[0], 0.0);
        assert_eq!(f[n - 1], 0.0);
    }

    #[test]
    fn heavier_gravity_favours_lower_modes() {
        // g raises every natural frequency, so a given tone maps to a
        // lower-order (coarser) mode.
        let light = LiquidSpec {
            gravity: 1.62,
            ..water()
        };
        let heavy = LiquidSpec {
            gravity: 24.79,
            ..water()
        };
        let k_light = dominant_mode(150.0, 0.025, &light).k;
        let k_heavy = dominant_mode(150.0, 0.025, &heavy).k;
        assert!(k_heavy < k_light, "k_light={k_light} k_heavy={k_heavy}");
    }
}
