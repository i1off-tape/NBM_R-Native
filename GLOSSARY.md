# NBM_R-Native Domain

Domain vocabulary and canonical terminology for the native Rust port of HBM's Nuclear Tech Mod on the Horizon voxel engine.

## Language

### Energy and Transport

**HE**:
The proprietary unit of electrical and potential energy used by NTM machinery, cables, and power networks.
_Avoid_: FE, RF, EU, Power, Energy Unit

**Cable Grid**:
An event-driven network component connecting HE generators and consumers, recalculated only on block placement or removal.
_Avoid_: Cable loop, Wire circuit, Power line

### Radiation and Hazards

**Rad**:
The canonical unit of absorbed ionizing radiation dose accumulated by living entities and tracked per world chunk.
_Avoid_: Radiation points, Dose level, Toxicity

**Rad/s**:
The rate of ionizing radiation emission per second from radioactive items, blocks, or contaminated chunks.
_Avoid_: Intensity, Radiation strength, Roentgen, Flux/s

**Fallout**:
Radioactive particulate contamination deposited into world chunks that slowly decays, diffuses to neighbors, and washes into soil with rain.
_Avoid_: Ash, Dirt, Nuclear waste, Soot

**Attenuation**:
The exponential reduction in radiation dose intensity caused by shielding blocks or hazmat armor between the emitter and the target.
_Avoid_: Protection factor, Blocking rate, Shield value

### Nuclear Physics and Reactors

**RBMK Column**:
An individual modular vertical block component (fuel rod, control rod, boiler, or moderator) comprising an RBMK reactor lattice.
_Avoid_: Reactor block, RBMK part, Core unit

**Moderation**:
The physical process of slowing fast fission neutrons down into thermal neutrons via moderator columns (graphite or beryllium).
_Avoid_: Neutron absorption, Reflection, Slowing

**Corium**:
The highly radioactive molten mass created when reactor columns exceed critical thermal limits and suffer catastrophic core meltdown.
_Avoid_: Melted fuel, Slag, Nuclear lava

### Engine and Modding Concepts

**Native Mod**:
An ahead-of-time (AOT) compiled Rust mod integrated into the Horizon engine binary through horizon-mixins AST injections.
_Avoid_: Plugin, Script mod, Coremod, Addon
