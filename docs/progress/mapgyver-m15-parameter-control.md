# Safe Parameter control admission (M15/1)

The reported crashes are reproduced against actual MapGyver Parameter declarations:

- Surface Opacity Animation at750ms returns an engine tick error:
  `value -1 is lower than min 0`.
- Video Speed Animation at17ms returns:
  `value 0.10661115427525991 does not align with step 0.01 from base -16`.

Both reject-policy failures originated at shared Parameter admission. Golden now projects
input, clamps/adapts its numeric range/step, then converts to the target kind. Integer
inputs remain wide until range admission; nonfinite data rejects before conversion.
Parameter writes/scripts and control/binding directions use this boundary. Authored
validation remains strict and owner invariants still reject atomically. Already legal
stepped values remain exact, preserving Undo/reload. A nonfinite computed oscillator
sample holds its last valid target rather than failing the engine tick.
The [architecture](../architecture/mapgyver-parameters.md) owns this contract.

## Current proof and limits — 2026-11-17

The baseline tests fail with the two messages above. After the fix, MapGyver's full library
passes734 tests,8 ignored. Golden Parameters passes31 tests,1 ignored, including scalar/
integer/vector/Color/CSS bounds, projected integer conversion, steps, exact endpoints,
nonfinite inputs, invalid constraints and idempotent legal values. Existing app tests now
verify finite clamping with exact values and Undo, retaining rejection of nonfinite and
invalid domain data. The updated behavior is intentional, not a weakened rejection test.

Golden Engine's full regression now passes700 tests,3 ignored. Two old assertions now
require numeric clamping while retaining exact live-state, history and persistence checks;
all18 feedback tests pass. This software rerun uses the existing `cargo test-fast` dev
profile, then its cached `golden_engine-7e37a68d81035a87.exe` for the full703-test suite.
It makes no native timing/profile qualification claim. Local terminal logs are
`target/m15-parameter-feedback.log` and `target/m15-parameter-engine-full.log`, both exit0.
The734-app/31-Parameter run preceded four additional app nonfinite-input table entries;
those extra rows still require execution in the next app test run.
Strict Clippy passes for MapGyver, Golden Parameters and Golden Engine libraries/tests
with `MapGyver/native-qualification` and `-D warnings` (exit0;
`target/m15-parameter-clippy.log`). Root and Golden Core `cargo fmt --all` pass.
No actual-app Inspector Animation run, fresh bundled build or M15/1 acceptance is claimed.

Next, run the final app nonfinite rows, rebuild and verify actual Inspector mode changes
for Surface/Composition opacity and Video Speed, history/reload,
bindings/scripts/transport, changed constraints and healthy sibling/invalid-sample behavior.
Keep M15/1 Active until its complete contract is verified. Do not open M15/2 yet.
The11:50–12:32UTC batch cutoff stopped new implementation/experiments. Existing validation
finished at16:15UTC after interrupted tool jobs were confirmed absent and resumed with
terminal logs. This is a software checkpoint. Performance, recovery and unusual-scenario
qualification remain second-pass work and do not expand this product fix.
