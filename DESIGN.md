# PROTOLITH — a process-simulating PBR rock texture generator

*Design document, v0.2 — working name negotiable (Protolith: the parent rock from which others are made; here, the parent process from which appearance is made).*

*v0.2 splits v0.1 into this file — the committed design — and [HORIZON.md](HORIZON.md) — the speculative program (v0.1's §15–§19 and milestones M9–M14). It also fixes two technical bugs found in review and rewrites the implementation strategy around a formally verified reference implementation. Changelog at the end.*

## 0. Thesis and scope

A rock's appearance is a fossilized record of its formation process. Where conventional procedural rock materials tune noise functions until they *look* right, Protolith coarsely simulates petrogenesis and lets the texture fall out. The payoff is twofold. First, physical accuracy becomes checkable: modal mineralogy, grain-size distributions, and reflectance spectra can be compared against real petrological data rather than eyeballed. Second, the parameter space collapses into a small set of *geological* parameters — bulk chemistry, cooling schedule, transport energy, metamorphic grade, weathering age — instead of dozens of opaque noise knobs.

v0.2 adds a third kind of checkability: the reference implementation is written in verified Rust (tactus), so the *structural* claims the simulation makes about itself — quotas met exactly, mass conserved exactly, every voxel owned by exactly one grain — are machine-checked theorems, not hopes (§11–§12).

The system is six process operators acting on a composition, plus a compiler:

```
crystallize[τ]    — nucleation-and-growth from melt          (igneous; volume/wall/point-seeded)
deposit[E]        — transport, settling, diagenesis           (clastic/chemical sedimentary)
deform[T, L]      — recrystallization under heat + kinematics (metamorphic; L = velocity gradient)
precipitate[chem] — reaction–transport from fluids            (replacement, rhythmic bands, accretion)
unmix[χ, M]       — subsolidus phase separation               (exsolution; Cahn–Hilliard)
weather[t, clim]  — chemical alteration + erosion             (surface aging)
compile           — petrofabric → PBR channel maps            (optics)
```

These compose. `deform` takes any fabric as protolith, so *marble = deform(deposit(carbonate))*, *gneiss = deform(crystallize(granitic melt))*, *quartzite = deform(deposit(quartz sand))*, *an agate geode = precipitate(vesicle(crystallize(basalt)))*. `deposit` can (v2) sample clasts from other generated rocks. Rock space is the closure of these operators over composition space — the rock cycle, literally, as an operator algebra. Everything else in this document is the concrete realization of those arrows.

Out of scope: migmatites (two-phase melt segregation — the one honest holdout, see §15), full thermodynamic phase equilibria (petrogenetic-grid lookups stand in), wet-surface optics. Essentially everything else in rock space is in scope, including biogenic rocks via content packs (§14, M7).

## 1. Architecture at a glance

```
params (JSON)
   │
   ▼
[A] Composition solver        chemistry → mineral assemblage + modal %, order
   │
   ▼
[B] Fabric kernels (B1–B5, freely nested)
   │        writes the PETROFABRIC BUFFER: per-voxel mineral, grain, orientation, age
   ▼
[C] Mesostructure             fracture network, differential relief, cleavage facets
   │◄──────────┐
   ▼           │  (C↔D alternate for "age" macro-iterations)
[D] Weathering ┘              alteration fronts, stain transport, dissolution
   │
   ▼
[E] Optics compiler           slice/shell → albedo, normal, roughness, F0, height, AO, SSS
```

Stage A is scalar arithmetic; B–E are data-parallel passes over the slab. Every pass is deliberately GPU-shaped (sweeps, ping-pongs, flood fills over regular grids), but the v1 implementation is a **verified CPU reference in tactus** (Lean-verified Rust), with a CUDA desktop port as phase 2, differential-tested bit-for-bit against the reference (§11). The v0.1 target of a single-file WebGPU HTML page is superseded; a browser build remains a possible eventual port, not a constraint.

## 2. The slab

The domain is a thin 3D voxel grid, default **512 × 512 × 32**, toroidal in X and Y (all distances, deposition, and stain transport wrap; output tiles seamlessly), open in Z. Z is depth.

**Occupancy is primary.** The exposed surface is the isosurface of voxel occupancy: stages C/D remove voxels (erosion, dissolution, crack aperture), so overhangs, cavities, undercut ledges, and honeycomb cells are representable. (v0.1 initially carried a single-valued height field `z_top(x, y)` as the surface; its own tafoni prediction required overhangs — a contradiction v0.1's third audit caught. v0.2 promotes the fix from patch to foundation and propagates it through §6–§8.) A height map `z_top(x, y)` survives as a *derived projection* — for output channels and for the few passes that genuinely operate columnwise (stain runoff). The surface is initialized flat ("sawn") and carved by stages C/D.

A physical scale `mm_per_voxel` (default 0.1 mm — hand-sample scale) makes texel density physically meaningful: a 2 mm feldspar phenocryst spans 20 voxels; Wentworth grain size ties in directly since diameter in mm = 2^(−φ). Choosing scale is choosing what is *resolved* versus *statistical* (see §5.4).

**Depth caveat (v0.2).** 32 voxels of depth = 3.2 mm at default scale — thinner than a coarse phenocryst. For phaneritic presets the slab is quasi-2.5D in Z: grains truncate at the slab faces (X/Y wrap toroidally, so no truncation there), which biases CSD statistics (§10) and weakens the consistent-re-slicing claim for coarse rocks. Mitigations: deepen the slab (64–128 in Z) for coarse presets, and compute CSDs from Z-untruncated grains only or apply a stereological correction. Recorded as a §13 risk.

Two output modes slice the slab differently. **Outcrop mode** projects the occupancy shell to 2D with full height/weathering channels (AO carries the cavity information the height projection can't). **Polished mode** takes an interior plane z = const with roughness clamped low and facets off — countertop granite, and incidentally the cleanest surface for validating albedo against photographs. Because the fabric is volumetric, re-slicing after a virtual fracture yields consistent interiors for free — the reason we chose a slab over a 2D field.

### 2.1 Petrofabric buffer (per voxel)

```
struct Voxel {
  mineral_id  : u8      // index into mineral DB; 0 = pore/air, 1 = glass
  grain_id    : u32     // unique per grain; hash(grain_id) seeds per-grain randoms
  orientation : 4×snorm16  // unit quaternion, crystal frame → slab frame
  t_impinge   : fixed16 // crystallization/deposition order (zoning, interstitial detection)
  alteration  : fixed16 // 0 = fresh … 1 = fully altered (written by stage D)
  aux         : fixed16 // phase-dependent: An-content zoning, cement flag, stain load
}
```

≈ 18 B/voxel in the Rust reference (a plain `#[repr(C)]` struct; the 16-bit fields are fixed-point, not f16 — see §12.2). GPU targets have no u8/f16 struct members without extensions, so the CUDA port packs into five u32 words (20 B). ~150–170 MB at 512²×32; offer 256²×16 preview (~19 MB). Voxels whose expected grain size falls below resolution use the statistical encoding of §5.4 instead of one-grain-one-id.

## 3. Mineral database

A single JSON table drives every downstream stage. Per mineral:

| field | type | consumed by |
|---|---|---|
| name, class | string | UI |
| density ρ | g/cm³ | A (wt% → vol%) |
| hardness | Mohs 1–10 | C (relief), D (feedback) |
| habit | polytope: face normals nᵢ + distances hᵢ | B1 (euhedral growth gauge) |
| cleavage | list of {plane normal (crystal frame), quality ∈ [0,1]} | C (facets), C (crack paths), E (roughness) |
| fracture | conchoidal / uneven / hackly | C, E |
| R(λ) | reflectance, 380–730 nm @ 10 nm, from USGS splib07 | E (albedo), validation |
| grain_bright | fitted slope of reflectance vs log grain size (from splib07 grain-size series) | E |
| n (+ k if opaque) | refractive index (complex for ore minerals) | E (F0 / metalness) |
| scatter | single-scattering albedo, mean free path | E (translucency) |
| alters_to | mineral_id + rate constant k_w | D |
| stain_yield | Fe³⁺ release per unit alteration | D |

**v1 mineral set (~16):** quartz, K-feldspar (orthoclase), plagioclase (An-parameterized), biotite, muscovite, hornblende, augite (pyroxene), olivine, calcite, dolomite, kaolinite (alteration product), hematite + goethite (stains), magnetite, pyrite, volcanic glass. This covers granite, granodiorite, diorite, gabbro, basalt, rhyolite, obsidian, pumice, sandstones, limestone, marble, slate/phyllite/schist, gneiss, quartzite.

Spectra are integrated offline (§8.1) so the runtime table carries precomputed linear-sRGB plus the retained spectrum for validation and relighting under non-D65 illuminants.

## 4. Stage A — Composition solver

**Input, two entry levels.** High level: a preset name resolving to bulk chemistry. Low level: oxide weight-percents (SiO₂, TiO₂, Al₂O₃, FeO, Fe₂O₃, MgO, CaO, Na₂O, K₂O, …).

**Igneous:** run the **CIPW norm** — the classical, fully deterministic ~40-step allocation algorithm that converts oxide chemistry into normative mineral weight fractions. Convert wt% → vol% via densities. Report the QAPF/TAS classification as a sanity readout ("this melt is a granodiorite").

**Norm→mode correction (v0.2 fix).** The CIPW norm is *anhydrous by construction*: its allocation sequence never produces biotite, muscovite, or hornblende — yet those minerals define the look of real granites, and the v1 mineral set, the Bowen order below, and the granite presets all depend on them. Normative ≠ modal is a classic petrological distinction, and v0.1 silently validated against the wrong one. Stage A therefore runs CIPW and then a hydrous correction in the **mesonorm** lineage (Barth; Mielke & Winkler's improved mesonorm for granitic rocks): given a water-activity parameter, reallocate normative orthoclase + mafic components toward biotite, and normative anorthite + pyroxene components toward hornblende, by fixed exchange stoichiometry. Both tables are reported — normative for classification, corrected modal for fabric quotas — and §10 validation targets the corrected modal assemblage. Genuinely dry presets (anhydrous basalt, obsidian) skip the correction; for them the norm really is the mode.

Crystallization *order* follows Bowen's reaction series: olivine → pyroxene → hornblende → biotite → plagioclase (calcic → sodic, giving An zoning) → K-feldspar → muscovite → quartz. The order list plus per-phase volume quotas is Stage A's entire output contract to B1.

**Sedimentary:** composition is specified directly as a clast recipe (e.g., quartz arenite: 95% quartz grains, silica cement; arkose: 60/25/15 quartz/feldspar/lithics, hematite-tinted calcite cement) or derived from a provenance preset. No solver needed; the physics lives in transport (B2).

**Metamorphic:** (protolith class, T, P) → assemblage via a small hand-built petrogenetic-grid lookup — e.g., the pelite sequence clay → chlorite → biotite → garnet zones. Honest full treatment is Gibbs-energy minimization (THERMOCALC-style); explicitly out of scope, and the lookup table is the declared approximation. The grade parameter also feeds B3's kinetics.

## 5. Stage B — Fabric kernels

### 5.1 B1: `crystallize` — sequential Johnson–Mehl on the slab

Nucleation-and-growth with impingement *is* a Johnson–Mehl tessellation — this is not a Voronoi-flavored aesthetic choice but the actual geometry of crystallization. Each seed s carries birth time t_s, growth speed g_s, phase p_s, orientation q_s, and habit polytope P (from the mineral DB). Its arrival time at voxel v is

```
T_s(v) = t_s + γ_P( R(q_s)⁻¹ · (x_v − x_s) ) / g_s
γ_P(y) = max_i ( nᵢ·y / hᵢ )        — Minkowski gauge of the habit polytope
```

The gauge makes early, unimpeded crystals genuinely **euhedral** — flat faces, correct interfacial angles — at the cost of a max over ~6–12 face planes per distance evaluation. An ellipsoidal metric (Mahalanobis distance under a habit tensor) is the cheap fallback. A voxel belongs to the seed minimizing T_s(v).

**Sequencing per Bowen.** Phases run in reaction-series order against their Stage-A volume quotas:

```
claimed ← ∅
for phase p in bowen_order:
    scatter N_p seeds in unclaimed space   // N_p from kinetics, below
    JFA over unclaimed voxels → per-voxel best (seed, T)
    t* ← quantile of T such that |{T ≤ t*}| = quota_p     // one histogram pass
    freeze voxels with T ≤ t*: write mineral, grain, orientation, t_impinge = T
```

Early phases stop at quota with idiomorphic faces intact; the final phase (quartz) inherits whatever interstitial space remains and is therefore **anhedral automatically** — exactly the granite fabric petrologists describe, produced by the mechanism that produces it in nature.

**Co-crystallization windows (v0.2 fix — ophitic/poikilitic texture).** Strictly sequential freezing cannot produce ophitic texture — plagioclase laths *enclosed within* single large pyroxene crystals, the defining fabric of dolerite and gabbro — because enclosure requires two phases growing through the same region during overlapping time windows. (No v0.1 audit caught this; "poikilitic" appeared nowhere in three passes.) The fix is cheap: adjacent Bowen phases may declare an overlapping window, running one JFA in which both phases' seed sets compete on arrival time. Nucleation-density/growth-rate contrast then does the petrology: seed pyroxene sparse and fast, plagioclase dense and slow, and each rare fast crystal envelops the many slow laths it overtakes — an oikocryst *is* a rarely-nucleated crystal that grew around everything. Quota accounting still applies per phase (per-phase quantile freeze as above). Sequential remains the default; overlap is a per-preset flag, and its (N, g) tuning is a §13 calibration item with enclosure statistics as the check.

**Kinetics → seed counts.** Rather than integrating nucleation/growth ODEs, use the JMAK/Avrami closed form: mean grain diameter scales as d ∝ (G/I)^(1/4) in 3D (constant rates, Avrami exponent 4), so per-phase seed density is N_p ∝ quota_p / d³ with d set by a monotone map from the cooling timescale τ (slow τ → growth-dominated → phaneritic; fast τ → nucleation-dominated → aphanitic). This map is where petrology is folded in; calibrate presets against published crystal-size-distribution (CSD) data (§10). A **two-stage τ schedule** — slow then fast — yields porphyritic texture with zero additional machinery: stage-1 phases become phenocrysts, stage-2 becomes groundmass. τ may also be a **field** τ(x) (e.g., a distance-to-contact map — one more JFA): chilled dike margins, pillow-basalt rinds, and aureole grain-size gradients at no structural cost (applications catalogued in HORIZON §16.2). **Zoning:** plagioclase An-content is a function of t_impinge, written to `aux`, read by E as an intra-grain albedo gradient.

**Vesicles:** before crystallization, carve Poisson-disk spheres (radius distribution from a volatiles parameter) as pore voxels; walls may later host amygdale fill (v2). **Glass:** if τ falls below a threshold, skip tessellation entirely — every voxel is `glass`, and obsidian's look comes purely from stage E (conchoidal fracture + dark F0).

**JFA implementation.** Standard jump-flooding (passes at strides N/2, N/4, …, 1) generalizes cleanly: propagate the (seed, T) pair minimizing arrival time instead of Euclidean distance; anisotropic gauges only require that each voxel evaluates T against candidate seeds, which JFA already does. Known JFA artifact rate is acceptable at slab resolution; a final 1-2 stride refinement pass cleans stragglers. Cost: log₂(512) ≈ 9 passes × ~8 phases over 8.4 M voxels.

**Seeding modes.** Volume-Poisson is the default, but two variants extend coverage substantially at near-zero cost. *Wall-seeded:* scatter seeds on a surface (crack walls, vesicle interiors, brine floor) and grow inward — **geometric selection emerges on its own**, as wall crystals whose fast growth axes point away from the wall outcompete their neighbors. This *is* comb quartz, geode linings (amethyst = wall-seeded quartz in a vesicle), and chevron halite growing up from an evaporite pan. *Point-seeded fibrous:* spherulites (devitrifying rhyolite, chalcedony) use an isotropic gauge but a radial internal fabric — store only the seed position and resolve per-voxel orientation as the radial direction at compile time, so a spherulite costs one seed.

**Melt kinematics.** Two pre-solidification options: `flow` applies a laminar shear warp to the composition and nucleation-density fields before crystallization — flow-banded rhyolite and obsidian are heterogeneous melt that moved, so the warp is the physics, not a domain-warping trick. `settle` lets each phase's just-frozen crystals fall through remaining melt via the B2 falling-sand pass before the next phase runs — honest crystal settling, yielding cumulate layering (modally graded gabbros) as a composition of `crystallize` with `deposit`'s machinery.

### 5.2 B2: `deposit` — transport statistics, settling, diagenesis

The physics enters through the *grain population*, which is where sedimentology actually lives. Sample grain diameters from a log-normal in φ-units: mean φ set by transport **energy** (high-energy rivers carry gravel; quiet basins collect clay), σ_φ = **sorting** (well-sorted beach vs poorly-sorted glacial till). Krumbein **roundness** ρ ∈ [0,1] increases with transport **distance**; realize each clast as a superellipsoid whose corner exponent interpolates angular → rounded with ρ.

**Settling.** v1 uses 2.5D ballistic deposition — a falling-sand pass, per-column atomics, dropping clasts from +z with jitter, resting them on the evolving height field, long axes settling near-horizontal with optional imbrication if a current direction is set. (This is deliberately the one stage built on falling-sand mechanics; it is the natural primitive here.) **Honesty note (v0.2):** per-column logic is exact only for clasts of ~voxel size; a superellipsoid spanning many columns is a rigid-contact problem even in 2.5D. v1 handles sand-scale clasts natively and settles large clasts (conglomerate cobbles) with a footprint-max + local-tilt heuristic — no interlocking, no bridging. Full 3D rigid-body packing is the v2 upgrade; the visual delta at texture scale is expected modest but is unproven (§13).

**Bedding** is nothing extra: let (mean φ, composition, current direction) vary with deposition time — a slow signal gives graded beds and laminae, a periodically re-inclined deposition surface gives cross-bedding.

**Diagenesis.** Compact (vertical affine squash, factor from burial parameter), then cement: pore voxels within radius r of clast surfaces convert to the cement mineral (quartz overgrowth / calcite / hematite film) until a target cementation fraction is met; surviving pores remain `mineral_id = 0` and matter optically (§8.1). Hematite cement is how red sandstone gets red — the color is diagenetic chemistry, not a tint knob.

**Carbonates (v1 minimal):** limestone = micrite (statistical calcite voxels, §5.4) with optional sparry patches; ooids (concentric growth shells around nuclei — a one-liner given the gauge machinery) and bioclasts are stubs.

### 5.3 B3: `deform` — Potts recrystallization under stress

Input: any petrofabric buffer as protolith, grade T (mapped to Monte-Carlo temperature and sweep count), and a **velocity gradient L = D + W** — symmetric stretch D plus spin W. The alignment term below uses ŝ = the most-compressive principal axis of D; the spin W additionally rotates grain quaternions a small increment per sweep, so simple shear (mylonites) and coaxial flattening (slates) are the same operator with different L. At high strain rate, add dynamic recrystallization: nucleate small new grains where a stored-energy proxy (accumulated rotation × hardness) is high — grain-size reduction and ribbon quartz, the mylonite signature, follow.

Run a Q-state **Potts model** on grain_id — the standard microstructure-evolution tool in materials science. Metropolis proposal: a voxel adopts a neighbor's (grain_id, orientation); energy is boundary energy plus an alignment term for platy phases:

```
E = Σ_<ij> J · [grain_i ≠ grain_j]  −  w Σ_{i platy} (c_i · ŝ)²
```

where c_i is the voxel's crystallographic c-axis (from its quaternion). Under compression, mica c-axes rotate toward ŝ, i.e., basal planes align perpendicular to compression — **foliation**, emerging from a two-term Hamiltonian. Race-free parallel sweeps use an 8-color 3D checkerboard. Phase transformations from Stage A's grid lookup swap mineral_ids at grade-dependent rates during the same sweeps.

The **grade knob** interpolates the real sequence: few sweeps + strong alignment → slate; more sweeps → phyllite → schist (coarsened micas); gneissic **banding** arrives by two honest routes, and no cheat is needed. Route one is free: a large share of natural banding is *transposed inherited layering*, and since `deform` accepts any protolith, `deform(deposit(...))` with bedding produces it with zero new machinery — the recursion earning its keep. Route two, metamorphic differentiation, is modeled as biased unmixing: a Cahn–Hilliard phase-separation term on felsic/mafic identity with mobility enhanced along the foliation plane (§15 shows the identical PDE also buys perthite exsolution and Widmanstätten patterns). The true mechanism of natural banding is genuinely debated in the petrology literature; shipping both routes, each with a defensible physical reading, is the accurate position.

With **no stress bias**, pure Potts coarsening of a calcite or quartz protolith relaxes toward a polygonal foam with ~120° triple junctions — which is precisely the granoblastic texture of marble and quartzite. The model produces the right texture because it is the accepted model *of* that texture.

### 5.4 Statistical voxels (the sub-resolution branch)

At 0.1 mm/voxel, basalt groundmass (grains ≪ 0.1 mm) and micrite cannot be resolved grain-by-grain. Whenever a phase's predicted d < ~2 voxels, skip explicit tessellation for it: voxels store a **phase histogram** (packed into aux + mineral_id conventions) plus a variance seed. Stage E then computes mixture optics — noting that intimate mineral mixtures combine approximately linearly in *single-scattering albedo*, not in reflectance (Hapke); v1 may ship linear-reflectance mixing with the Hapke-space upgrade noted. Effective roughness rises and per-grain facet logic is disabled for statistical voxels. This branch is what makes basalt, micrite, and slate matrices honest rather than accidentally phaneritic.

### 5.5 B4: `precipitate` — reaction–transport from fluids

The constructive counterpart of weathering: stage D's reaction–diffusion machinery writing minerals instead of destroying them. Three modes, one operator.

**Replacement fronts (metasomatism).** A solute concentration field c infiltrates from boundaries and cracks by advection–diffusion; where c exceeds a threshold and the host mineral is in the reactive set, swap mineral_id at rate k. Because cracks are fast-paths (§6), reaction rims advance from the fracture network inward — which is precisely why serpentinite has its **mesh texture** of relict olivine cores inside serpentine rims: the pattern is the transport topology, for free. Dolomitization is the same front with calcite→dolomite; skarn zoning falls out of multiple thresholds keyed to c ranges (sequential reaction zones behind a moving front); chert nodules are silica replacement in limestone.

**Rhythmic precipitation (Liesegang).** Classic supersaturation–nucleation–depletion: solute A diffuses into host containing B; precipitate forms where [A][B] exceeds K_sp times a supersaturation factor, nucleation locally depletes both, the front advances until threshold is re-reached — discrete bands with geometrically increasing spacing. **Self-testing physics:** the Jablczynski spacing law (x_{n+1}/x_n → const) is a measurable output the simulation must reproduce, giving a validation target with zero extra work. Liesegang-ringed sandstone, and the chemistry half of agate.

**Surface accretion.** Level-set growth of an interface along its normal, rate modulated by a time-varying chemistry signal → laminae. Travertine and speleothem banding (accrete outward), crustiform vein fill (accrete inward from both walls, alternating chemistry), ooid cortices (accrete around nuclei, agitation resets orientation). Redistancing reuses the JFA pass — the accretion mode is jump-flooding pointed at a different problem. **Agate** is the flagship composition: vesicle → inward accretion laminae → fibrous chalcedony (point/wall-seeded fibrous mode, §5.1) → optional comb-quartz core. Every stage is an existing primitive.

### 5.6 B5: `unmix` — Cahn–Hilliard exsolution

Subsolidus phase separation, attachable as a cooling tail to `crystallize` or running inside `deform`. Order parameter φ (e.g., Or–Ab fraction within alkali feldspar grains; coarse-grained felsic–mafic identity for gneissic differentiation) evolves by

```
∂φ/∂t = ∇·( M ∇μ ),   μ = f′(φ) − κ ∇²φ,   f = double-well
```

with **anisotropic mobility M**: for perthite, lamellae are crystallographically controlled, so M is a tensor aligned per-grain via the stored quaternion and a per-mineral exsolution plane in the DB; for gneiss, M is enhanced in the foliation plane and φ couples back to Potts phase swaps (voxels flip mineral toward the local φ majority). Numerics: explicit stabilized finite differences in conservative flux form (§12 — conservation is then *exact*, and proved), single fixed-point φ channel, 10²–10³ ping-pong steps at slab scale. **Self-testing physics again:** linear spinodal analysis fixes the fastest-growing wavelength λ* = 2π·sqrt(2κ/|f″|), and conserved coarsening must follow the Lifshitz–Slyozov t^(1/3) law — both are assertions the implementation checks about itself at runtime (§12 tier 2). Presets that fall out: perthitic granite (automatic, since alkali feldspar + slow cooling is the default granite path), and an **iron meteorite** — kamacite lamellae unmixing from taenite on the octahedral plane set, rendered in polished mode: etched Widmanstätten as a two-operator program.

### 5.7 Pyroclastic rocks

Tuff is `deposit` with a clast population of glass shards, pumice fragments, and free crystals. **Welding** is the diagenesis squash with temperature-dependent clast softness: hot glassy clasts flatten preferentially into fiamme, producing eutaxitic texture; cold tuff just compacts. One softness-by-phase table entry, no new operator.

## 6. Stage C — Mesostructure

*(v0.2: this stage now operates on the occupancy isosurface throughout — see §2.)*

**Fracture network.** Cracks are least-cost paths on the voxel graph where the local edge cost encodes toughness: cost drops sharply when the step crosses a grain boundary (intergranular fracture) and when the step direction lies in a cleavage plane of the local grain (intragranular fracture along cleavage, using the per-voxel quaternion); it is high through unbroken strong minerals. A toughness-ratio parameter picks the intergranular/intragranular regime. Seed crack tips at slab borders and high-stress points, grow by Dijkstra/percolation with branching probability. A crack **carves occupancy** along its path — an aperture profile widening with accumulated length — rather than displacing a height value, so crack walls are real interior surfaces: re-slicing across them shows consistent interiors, and B1's wall-seeded mode can line them (veins) later. Crack voxels are also registered as weathering fast-paths for stage D. For quartz/glass, decorate fracture faces with concentric ripple arcs around the initiation point — conchoidal fracture is this and only this.

**Differential relief.** Iterate erosion on the occupancy surface: remove surface voxels at rate ∝ exposure / hardness(mineral), where exposure is local openness (the AO bake generalizes the curvature term and lets undercuts deepen rather than smooth away). Soft biotite recesses; quartz stands proud. Hardness is read *after* alteration (kaolinized feldspar erodes like clay, not like feldspar) — this is one leg of the C↔D feedback loop.

**Cleavage facets.** For surface voxels whose mineral has cleavage: rotate the cleavage-plane normals by the grain quaternion, snap the local surface normal toward the nearest cleavage normal with strength ∝ cleavage quality. Fresh-broken feldspar and mica get flat, coherent, glinting faces per grain; quartz (no cleavage) stays conchoidal. Crystal symmetry is handled by storing the full set of symmetry-equivalent planes per mineral in the DB (mica: 1 perfect basal plane; feldspar: 2 near-orthogonal; calcite: 3 rhombohedral).

## 7. Stage D — Weathering

Per-voxel alteration integrates a first-order front:

```
da/dt = k_mineral · w(x) · (1 − a)
w(x)  = exp(−d_surf/δ) · (1 + β·crack(x)) · (1 + γ·(1 − AO(x)))
```

where d_surf is distance to the occupancy surface — a distance transform, i.e., one more JFA (the solver library again). The moisture proxy w concentrates weathering at the surface, along cracks, and in concavities — the AO term is the trick: bake ambient occlusion on the isosurface *before* weathering and reuse it as a water-retention prior (hollows stay damp), then re-bake after for the output channel. Because the surface is an isosurface, shadowed hollows can now undercut — the tafoni-emergence prediction (HORIZON §15.5) is testable rather than vacuously false. This deliberately replaces the full Darcy-flow + mineral transport/dissolution/recrystallization simulation of Dorsey et al. 1999 with a surface-field approximation; their paper remains the reference for the volumetric version and the upgrade path if slab-interior weathering ever matters.

**Chemical rules are table-driven** (mineral DB `alters_to`): feldspar → kaolinite (whitens, roughness ↑, hardness ↓ → faster relief erosion); biotite, pyrite, olivine, augite → oxidize, releasing Fe³⁺ (`stain_yield`) — this is why basalt and mafic minerals go brown first; calcite → dissolves outright, removing voxels (pitting; with a drainage-direction bias, karren grooves).

**Stain transport.** Released iron obeys an advection–diffusion–deposition pass on the projected surface: velocity = downslope gradient of the height projection, plus diffusion, with distance-decaying deposition into `aux`. Result: rust streaks bleeding below every biotite cluster and pyrite cube — the single most recognizable signature of real weathered granite. Ping-pong passes, identical machinery to any reaction–diffusion toy.

**Age loop.** Alternate C and D for 4–8 macro-iterations under an `age` parameter: weathering softens, erosion strips, fresh material exposes. Lichen/biofilm is explicitly v2 (DLA or Gray–Scott colonies seeded by moisture, masked by substrate chemistry).

## 8. Stage E — Optics compiler

Resolve the slab (outcrop shell = occupancy isosurface, or polished slice) into 2D channel maps. Every texel gathers its voxel's fresh mineral and alteration product, lerped by a, then:

**8.1 Albedo (spectral, offline-integrated).** Per mineral: take R(λ) from USGS splib07 (measured 0.2–200 μm; we retain 380–730 nm), multiply by the D65 illuminant, integrate against the CIE observer functions to XYZ, convert to linear sRGB. Precomputed into the DB; runtime is a table lookup + lerps. Retaining spectra enables (a) relighting under arbitrary illuminants without metamerism errors and (b) direct validation against splib07's *rock* spectra (§10). Corrections applied per texel: **grain-size brightening** (finer grains scatter more; slope fitted per-mineral from splib07's grain-size series — the library ships these on purpose), **porosity darkening** (pore fraction traps light), **zoning** (An-content gradient in plagioclase), **stain overlay** (hematite/goethite spectra composited by `aux` load — thin-coating composite, not opaque replacement).

**8.2 Specular F0 / metalness.** Dielectrics: F0 = ((n−1)/(n+1))² from the DB index — quartz n≈1.55 → 0.046; nearly all rock lands in F0 ∈ [0.035, 0.06], which is itself a physical-accuracy statement (rocks are not shiny, they are *faceted*). Ore minerals with complex IOR use the conductor Fresnel F0(λ) = ((n−1)² + k²)/((n+1)² + k²) evaluated per RGB → pyrite genuinely reads brassy-metallic. Metalness map is therefore *not* identically zero, just almost.

**8.3 Roughness.** Base per mineral by surface type: cleavage faces low α (glossy facets), conchoidal moderate with ripple modulation, granular/statistical high. Alteration lerps toward the product's roughness (kaolinite coat ≈ 0.85+). Sub-texel grain size adds micro-roughness. Polished mode overrides to a uniform low α with per-mineral polish response.

**8.4 Normal / height / AO.** Height = normalized projected depth of the occupancy surface; normal = occupancy-gradient normal composited with intra-grain cleavage-facet normals; AO = horizon-based bake on the isosurface (also consumed by D, order per §7 — and it carries the cavity information the height projection cannot).

**8.5 Translucency.** Per-mineral scattering parameters × local grain size → a thickness/translucency channel. Matters visibly for calcite (marble is *the* SSS material), quartzite edges, gypsum; ignorable elsewhere.

**8.6 Glints — declared limitation.** Real rock sparkle is discrete specular events from individual cleavage facets; a roughness map cannot represent it under a moving light. Options, in ascending fidelity: (a) rely on facet normals + low roughness (adequate when texel ≈ facet, i.e., coarse rocks at hand-sample scale); (b) emit an auxiliary glint density/orientation map for a dedicated glint BRDF on the renderer side; (c) accept matte output for weathered outcrop, where nature has already destroyed the facets. v1 ships (a) + the map for (b).

## 9. Parameter schema

```jsonc
{
  "slab":   { "size": [512,512,32], "mm_per_voxel": 0.1, "tile_xy": true, "seed": 0xB33 },
  "rock": {
    "op": "deform",                       // crystallize | deposit | deform | precipitate
                                          // (unmix + settle/flow attach as sub-blocks)
    "T": 550, "P": 4, "stress": { "axis": [0,0,1], "mag": 0.7 },
    "protolith": {                        // operators nest — the rock cycle is recursive
      "op": "crystallize",
      "chemistry": { "preset": "granite" },        // or explicit oxide wt%
      "cooling":   [ { "tau": 1e5 } ],             // list of stages; 2 stages → porphyritic
      "volatiles": 0.0
    }
  },
  "weathering": { "age": 0.4, "humidity": 0.6, "cut": "outcrop" },   // or "fresh" | "polished"
  "output": { "channels": ["albedo","normal","roughness","height","ao","f0","translucency"],
              "resolution": 2048 }
}
```

The recursion is the design's center of gravity: the schema *is* the operator algebra of §0, and every classic rock name is a short program. Ship ~28 presets as named programs: granite (perthitic by default), granodiorite, gabbro (ophitic), cumulate-layered gabbro, basalt, obsidian, flow-banded rhyolite, porphyritic rhyolite, pumice, welded tuff, pegmatite (flagged: kinetics map overridden, see §15), quartz arenite, arkose, red sandstone, conglomerate, breccia, shale, limestone, coquina, dolostone, travertine, chert, rock salt (chevron), marble, slate, schist, mylonite, gneiss, quartzite, serpentinite, agate geode, Liesegang sandstone, anthracite, iron meteorite (Widmanstätten).

## 10. Validation — receipts for "physically accurate"

**By construction — and now by proof:** output modal % equals the Stage-A modal target (post-mesonorm, §4) exactly; the quota threshold enforces it, and in v0.2 that enforcement is a machine-checked theorem, not a code comment (§12).

**Fabric statistics:** crystal-size-distribution (CSD) plots — log population density vs size — are a standard petrological instrument with published curves; generate them from the grain map (Z-untruncated grains only, per §2) and compare slopes against literature values for given cooling regimes. Grain-size/sorting histograms for sedimentary output check directly against the sampled Wentworth targets (round-trip test of the deposition stage).

**Color:** splib07 contains measured spectra of whole *rocks*, not only minerals. Generate granite, integrate the polished-mode albedo back to a spectrum (we kept spectra for exactly this), and compare against a measured granite spectrum; likewise render swatches under D65 next to hand-sample photographs.

**Virtual thin sections (the showpiece).** The slab already stores per-grain crystallographic orientation, and the mineral DB can carry birefringence. A 30 μm slice between crossed polarizers is then renderable: interference color from retardation = birefringence × thickness via the Michel–Lévy chart, extinction as the stage rotates. Comparing synthetic XPL images against real thin-section micrographs is simultaneously the strongest qualitative validation available and an extremely beautiful demo mode. Petrographers could, in principle, grade the output with their actual professional toolkit.

## 11. Implementation strategy — verified reference first, CUDA second

Two implementations, one contract.

**Phase 1 — the reference: verified Rust (tactus).** The entire A–E pipeline is a **pure verified core**: `params → petrofabric buffers → channel maps`, no I/O, no floats (§12.2), every pass a plain function over slab arrays, verified with the Lean backend. Around it, a deliberately thin unverified shell: JSON param parsing, PNG/EXR channel writing, a preview window, and a parallel driver. The shell contains no simulation arithmetic; it is enumerated in one module and reviewed by eye.

**Parallelism seam.** The same structure that makes GPU passes race-free — 8-color checkerboards, per-stride JFA passes, disjoint tiles — partitions CPU work into provably disjoint chunks. Verified *sequential* kernels are mapped over chunks by a rayon driver whose only job is scheduling; disjointness of the chunk decomposition is itself a proved lemma, so the driver has nothing semantic to get wrong.

**Phase 2 — the port: CUDA (desktop app).** Every pass is already GPU-shaped; the pass inventory survives from v0.1 unchanged: JFA (9 passes × ≤8 phases), quota histogram (1 pass/phase), deposition (falling-sand or the large-clast heuristic), Potts sweeps (8-color checkerboard, O(10²–10³) at grade), crack frontier expansion, relief steps (≤16), weathering + stain ping-pong (O(10²) × age iterations), C–H unmix (explicit, 10²–10³ steps), precipitate (RD ping-pong; level-set accretion via JFA redistance), channel resolve (1 pass/channel), preview renderer. The port is a translation, not a redesign.

**The bridge — bit-for-bit differential testing.** All randomness comes from counter-based hashes keyed (seed, voxel, pass), and all core arithmetic is integer/fixed-point (§12.2). Integer arithmetic is exactly portable — unlike floats, where FMA contraction and libm differences guarantee CPU/GPU drift — so the CUDA port must reproduce the verified reference **bit-for-bit, stage by stage**. Golden outputs recorded per stage per preset make the reference an oracle: the proofs live once, in the reference; the port inherits them by exact agreement. This also preserves v0.1's shareable-preset determinism (identical params → identical rock), now doing double duty as the porting contract.

**Perf expectations, honestly.** Preview slab (256²×16 ≈ 1M voxels): JFA in seconds; Potts/C–H at high grade are the long poles, tens of seconds to a few minutes single-threaded, divided by cores under the driver. Full 512²×32 metamorphic paths: minutes on CPU. Acceptable for an oracle and for validation sweeps; interactivity is what the CUDA port is for. (The v0.1 "well under a second" figure belongs to the GPU target, not the reference.)

## 12. Verification plan

The correctness story has three tiers; formal verification is the new bottom one.

- **Tier 1 — proved (tactus): structural invariants.** Machine-checked, hold for every input and every seed. Listed below.
- **Tier 2 — runtime self-tests: statistical physics.** Jablczynski band-spacing ratio, spinodal wavelength λ*, Lifshitz–Slyozov t^(1/3) coarsening, CSD slope sanity, quota re-counts. These are properties of the physics *at given parameters*, not theorems about code; the harness asserts them at runtime and a failure is a physics-tuning bug, not a soundness bug.
- **Tier 3 — offline validation: reality.** splib07 spectra, hand-sample photographs, thin-section micrographs (§10).

The discipline: **prove structure, not pixels.** No theorem will say "this looks like granite"; the theorems say the machinery cannot lie about what it did — quotas exact, mass conserved, nothing double-owned, nothing out of range. Tier 1 is precisely the layer where "subtle thingies that need to be exactly right" live, and it is cheap to specify because the design already states its invariants prose-form.

### 12.1 Proof obligations by stage

| Stage | Proved invariants |
|---|---|
| Slab infra | toroidal index/neighbor arithmetic (wrap lemmas); voxel encode/decode roundtrip; counter-hash is a pure function of (seed, voxel, pass) |
| A (CIPW + mesonorm) | oxide-mass conservation end-to-end; non-negativity of every allocation step; vol% partition of unity after density conversion; termination (straight-line allocation) |
| B1 (crystallize) | every voxel owned by exactly one (phase, grain) or pore/glass — a partition theorem; per-phase claimed count == quota exactly (the quantile threshold's defining property); frozen voxels satisfy T ≤ t*; gauge γ_P well-definedness (positivity, homogeneity) |
| B2 (deposit) | support invariant (no floating clasts); clast-voxel bookkeeping (placed = sampled − out-of-domain); monotone surface growth during settling; cementation fraction met exactly |
| B3 (Potts) | checkerboard color classes are pairwise non-adjacent — the race-freedom theorem; Metropolis ΔE agrees with the Hamiltonian; per-mineral voxel counts invariant under pure coarsening (declared transformation swaps excepted) |
| B4 (precipitate) | reacted set grows monotonically; threshold semantics; front connectivity to its sources |
| B5 (C–H) | **Σφ conserved exactly per step** (conservative flux form telescopes — an integer identity, the flagship proof); φ stays in its clamped range |
| C (mesostructure) | crack paths are connected voxel paths with correct accumulated cost; carving only removes occupancy (monotone); facet normals remain unit |
| D (weathering) | a ∈ [0,1] preserved by the integrator; stain mass bookkeeping (released = deposited + in-flight + decayed) |
| E (compile) | channel ranges (albedo, roughness ∈ [0,1]); color-matrix constants match their spec definition; statistical-voxel mixture weights sum to 1 |

### 12.2 Number representation

**The core is fixed-point/integer throughout.** Three reasons, each independently sufficient:

1. **Verifiability.** tactus/Verus reason about integer arithmetic natively; f32/f64 are unsupported in exec code and float reasoning is an order of magnitude harder. In-workspace prior art: the verified GpuFixedPoint Ring, LimbOps, and the fixed-point→WGSL pipeline from the Mandelbrot project.
2. **Exact conservation becomes provable.** C–H in conservative flux form conserves Σφ as a *literal integer invariant* — floats cannot even state that property, let alone prove it. Same for stain mass and clast bookkeeping.
3. **Exact portability.** Integer ops are bit-identical across CPU and CUDA; the §11 differential test depends on this.

Per-field formats (arrival times, φ, concentrations, alteration) are chosen per stage with documented Q-format and saturation semantics; dynamic-range analysis for the PDE stages (C–H stability under explicit stepping, arrival-time range across phases sharing a clock) is a named §13 risk, not an afterthought.

**The float seam.** Where floats genuinely earn their keep — CUDA inner loops later, possibly stage-E tone mapping — **lean-flocq** is the designated bridge: IEEE-754 semantics, error-free transformations, and Shewchuk expansion arithmetic are already ported there, and the expansion machinery gives exact sign evaluation (robust geometric predicates) if crack routing or facet geometry ever needs it. Floats enter only through that door, with specs attached.

### 12.3 Practicalities

The crate lives in this repo (`material-synthesis/protolith`) with a crate-local `check.sh` in the tactus-group-theory pattern (Lean backend). The Lean backend rewards small modules and short lemmas — one pass per module, proof helpers in separate files. CI asserts "0 errors," never pinned verified-function counts (they drift). `#[verifier::external_body]` / `assume` / `admit` are forbidden in the core per standing policy; the unverified surface is exactly the I/O shell, named as such.

## 13. Risks and open questions

**Gneiss banding** (§5.3): decision made — honest Cahn–Hilliard segregation plus the inherited-layering route; the quota-modulation cheat is dropped. Risk shifts to C–H tuning (mobility anisotropy, sweep budget) and to validating band wavelength against measured gneisses. **Co-crystallization windows** (§5.1): the ophitic (N, g) contrast is a calibration task; check = enclosure statistics (fraction of plagioclase laths fully enclosed) against real dolerite point counts. **Deposition** resists clean parallelism and, for multi-column clasts, clean *mechanics*: the 2.5D falling-sand compromise plus the large-clast heuristic trades packing fidelity for tractability — probably invisible at texture scale, but unproven. **Kinetics calibration** (τ → grain size) is a hand-built map wearing physics clothing until CSD-fitted; presets should record their provenance. **Intimate-mixture optics** for statistical voxels: linear-reflectance mixing is wrong in a known direction (Hapke single-scattering-albedo space is the fix); decide after seeing basalt v1. **Quaternion storage** at 4×snorm16 may band the facet normals; octahedral-frame encoding if visible. **Fixed-point dynamic range** (v0.2): C–H explicit stepping needs a stability-analyzed Δt and a φ format with headroom; multi-phase arrival times sharing one clock need documented saturation. **CPU Potts/C–H throughput**: at high grade the reference is minutes-slow; if that drags iteration, run calibration at preview resolution — never weaken the oracle. **Verification cost discipline** (v0.2): tier 1 is scoped to structural invariants precisely so proofs don't eat the project; any proposed theorem about *appearance* is out of scope by policy. **C–H stiffness**: explicit scheme needs small Δt (budget sweeps; assert t^(1/3) at runtime). **Level-set accretion** aliases laminae thinner than ~2 voxels — clamp band frequency to resolution or supersample the accretion field locally. **Serpentinization volume change** (~+40% in nature) is ignored; if mesh textures look too tight, a mild dilation term is the honest patch.

## 14. Milestones

**Committed (the go/no-go arc):** M0–M2 exercise every architectural layer (A, B1, C, D, E) on one rock family before anything else is built.

- **M0:** slab infra + JFA-JMAK + 3 minerals + flat optics → recognizable pseudo-granite. *Verification:* slab/indexing lemmas; B1 partition + quota theorems.
- **M1:** CIPW + mesonorm + Bowen sequencing (incl. co-crystallization windows) + spectral albedo → true granite/granodiorite/gabbro (ophitic) family, polished mode, first color validation. *Verification:* Stage-A conservation suite.
- **M2:** mesostructure + weathering loop on the isosurface → outcrop granite with rust streaks (the money shot). *Verification:* C/D invariants; a ∈ [0,1]; stain bookkeeping.

**Planned continuation:**

- **M3:** deposit kernel → sandstones, shale, limestone, tuff. *Verif:* support + clast bookkeeping.
- **M4:** Potts + kinematic deform + C–H unmix → marble, slate, schist, mylonite, gneiss, perthite, iron meteorite. *Verif:* exact-conservation flagship; checkerboard race-freedom.
- **M5:** `precipitate` (fronts, rhythms, accretion) + seeding modes → agate geodes, serpentinite, dolostone, travertine, Liesegang sandstone, veins, rock salt. *Verif:* front monotonicity; *runtime:* Jablczynski.
- **M6:** melt kinematics (flow banding, cumulates) + welded tuff + spherulites.
- **M7:** content packs — bioclast SDF library (coquina, fossiliferous limestone), organic macerals with rank→reflectance mapping (peat through anthracite), lichen colonies as an optional stage-D pass.
- **M8:** virtual thin sections + full validation suite + preset gallery; CUDA port lands (the differential harness records golden outputs from M0 onward).

**M9–M14** (fracture-physics pack, Laplacian growth, instrument targets, the optical volume, Lapidary, the ab initio basement) live in [HORIZON.md](HORIZON.md) — deliberately outside the committed roadmap, with a defined promotion path.

## 15. Coverage audit — what the four verbs miss

Auditing by *process* rather than rock name, the gaps collapse into a short list, and most of them are one PDE and one verb away.

**Unmixing (exsolution).** Perthite — the albite lamellae that stripe nearly every slowly cooled alkali feldspar, visible in hand sample — is subsolidus phase separation, as are Widmanstätten patterns in iron meteorites and various fine intergrowths. The model is Cahn–Hilliard, a ping-pong-able fourth-order PDE. Since the honest gneiss-banding decision (§5.3) already commits to a C–H term, exsolution textures come nearly free: same primitive, different phase pair and mobility tensor. Add `unmix[χ, M]` as a sub-operator of `deform`/`crystallize` cooling tails.

**Constructive precipitation.** Agate banding, travertine and speleothem laminae, Liesegang rings in sandstone, concretions, dolomitization fronts, and the metasomatic family (serpentinite mesh texture, skarn) are all precipitation/replacement from fluid — reaction–diffusion run *constructively*, i.e., stage D's machinery writing minerals instead of dissolving them, sometimes coupled to surface-normal accretion. This is the genuine fifth verb: `precipitate[chem, front]`. Notably, hydrothermal **veins and geodes need no new verb at all**: seed `crystallize` on crack or vesicle walls and geometric selection emerges on its own — wall-nucleated crystals whose fast axes point inward outcompete the rest, which is exactly comb quartz; an amethyst geode is `vesicle + wall-seeded crystallize`, a composition of existing machinery.

**Melt-present flow.** Flow-banded rhyolite is a pre-solidification shear warp of the composition field (cheap). Cumulate layering is `crystallize` composed with settling — i.e., `deposit` acting on crystals inside melt (a composition, not a new verb). Welded tuff is `deposit` of glass shards plus hot compaction with fiamme flattening (the diagenesis squash, anisotropic). **Migmatites are the honest holdout**: partial melt segregation is two-phase flow, and no cheap primitive fakes it defensibly. Declared out of scope.

**Kinematics.** `deform` currently takes a compression axis; mylonites (ductile shear zones: ribbon quartz, extreme dynamic grain-size reduction) need the full stress tensor with a rotational component. Parameter generalization, not new physics.

**Content, not physics.** Fossiliferous limestone and coquina need a bioclast shape library; coal needs organic macerals in the DB (amusingly, vitrinite *reflectance* is the literal industry proxy for coal rank — the optics compiler would be speaking the native language); lichen ships as an optional stage-D colony pass. These are asset problems the architecture already accommodates, scoped as the M7 content pack.

**Parameter-regime breaks.** Pegmatite fabric is mechanistically covered (very low nucleation density) but the τ → grain-size map is wrong for it — pegmatites are flux-driven (H₂O/Li/B/F), not slow-cooled, and their crystals exceed the slab at hand-sample scale anyway. Spherulitic devitrification in glassy rocks needs a radial-fibrous growth mode: a small extension to the gauge machinery, not a redesign.

**Audit conclusion:** `precipitate` and `unmix` are now first-class operators (§5.5–5.6), kinematic `deform`, seeding modes, melt kinematics, and pyroclastics are folded in (§5.1, §5.3, §5.7), and biogenic rocks are scoped as content (M7). A fourth catch arrived in v0.2 review: **co-crystallization textures (ophitic/poikilitic)** — missed by all three v0.1 audits, now folded into §5.1. Coverage is therefore most of rock space — though the second and third audits ([HORIZON.md](HORIZON.md)) find further corners. The irreducible residue: **migmatite** (two-phase melt segregation — genuinely hard physics, honestly declined) and full Gibbs-minimization phase equilibria (the petrogenetic-grid lookup is the declared approximation).

## References / reading

Dorsey, Edelman, Jensen, Legakis, Pedersen, *Modeling and Rendering of Weathered Stone*, SIGGRAPH 1999 — volumetric slab weathering: moisture flow, mineral transport/dissolution/recrystallization, SSS rendering. Kokaly et al., *USGS Spectral Library Version 7* (splib07a/b, USGS Data Series 1035) — measured mineral/rock reflectance 0.2–200 μm incl. grain-size series; download via USGS ScienceBase. Soulié et al., *Modeling and Rendering of Heterogeneous Granular Materials: Granite Application*, CGF 2007. Avrami/JMAK kinetics + Potts microstructure evolution: any materials-science microstructure text. CIPW norm: standard petrology references (Cross, Iddings, Pirsson, Washington 1902 lineage; modern restatements widely available). Barth, *mesonorm* lineage + Mielke & Winkler 1979 (improved mesonorm for granitic rocks) — the norm→mode hydrous correction of §4. Marsh, *Crystal Size Distribution (CSD) in rocks and the kinetics and dynamics of crystallization* — the CSD validation target. Palandri & Kharaka, *A compilation of rate parameters of water–mineral interaction kinetics* (USGS) — the empirical weathering-kinetics source. In-workspace: lean-flocq (IEEE-754 + EFT + Shewchuk expansions in Lean 4), the GpuFixedPoint/LimbOps verified fixed-point stack, and the tactus-group-theory crate-local check.sh pattern.

## Changelog v0.1 → v0.2

1. **Split**: §15–§19 (second/third audits, Lapidary, optical volume, ab initio) and milestones M9–M14 moved to HORIZON.md. The committed roadmap is M0–M8; the hard commitment is M0–M2.
2. **Mesonorm fix (§4)**: CIPW is anhydrous and can never yield biotite/hornblende; added the norm→mode hydrous correction. §10's by-construction claim now targets the corrected modal assemblage.
3. **Co-crystallization windows (§5.1)**: sequential freezing can't make ophitic/poikilitic texture; overlapping phase windows added. Missed by all three v0.1 audits.
4. **Occupancy isosurface promoted to primary (§2)** and propagated through §6/§7/§8 (v0.1 applied the third-audit fix only locally); height field demoted to derived projection.
5. **Slab-depth caveat (§2)**: 32 voxels = 3.2 mm is quasi-2.5D for phaneritic rocks; CSD stereology note.
6. **τ(x) fields (§5.1)**: promoted from HORIZON §16.2 (one line, no structural cost).
7. **Implementation strategy rewritten (§11)**: verified tactus reference + thin unverified shell + CUDA port with bit-for-bit differential testing, replacing the single-file WebGPU HTML target (and its 2–3 kLOC estimate, which was off by several×).
8. **Verification plan added (§12)**: three-tier correctness stack; per-stage proof obligations; fixed-point core with lean-flocq as the float seam.
9. **Deposition mechanics honesty (§5.2, §13)**: multi-column clasts are a contact problem; v1 heuristic named as such.
10. **Milestones recut (§14)** with per-milestone verification deliverables.
