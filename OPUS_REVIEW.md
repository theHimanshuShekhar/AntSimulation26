Code Review: AntSimulation

Bugs / Correctness Issues

1. src/ant.rs:48-59 — deposit unreachable code / broken tuple destructure. The match returns (&mut Vec<f32>, &mut Vec<f32>, &mut Vec<f32>) into names intensity, dx, dy. This is fine, but the let n = self.home.len(); if idx >= n bound-check only
   checks home, not food. Harmless today since all six Vecs are the same length, but fragile.
2. src/ant.rs:228-230 — repeated distance computation. Inside min_by, a.distance(pos) and b.distance(pos) are called per comparison; this recomputes sqrt N log N times. Cache distance_squared once per food, and also use distance_squared in the filter
   (compare vs SEEK_RADIUS \* SEEK_RADIUS).
3. **src/main.rs:148 let \_ = placed;** — dead write. placedis never read and can be removed (plus the whole 10-try loop could simplycontinue` on success).
4. src/main.rs:109 — sqrt as usize is imprecise. dist = (dx²+dy²).sqrt() as usize should compare squared distances: if dx*dx + dy*dy >= MIN\*MIN, avoiding sqrt and truncation bugs.
5. src/ant.rs:213-218 — sensor tie-break favors left. if ahead >= left && ahead >= right then else if left > right. When left == right (both nonzero while ahead is smaller), ants always pick right (SENSOR_ANGLE). Likely fine, but non-obvious;
   consider adding a note or symmetric random tie-break.
6. \*\*src/ant.rs:194-281 — Local<Option<SmallRng>> re-seeded per system invocation is fine, but rand::random::<f32>() inside handle_collision (line 134-136) uses the thread RNG, bypassing the SmallRng. Mixing is wasteful; pass the SmallRng into
   handle_collision.
7. src/main.rs:214-219, 167-172 — mesh/material allocated each respawn batch. ant_respawn_system calls meshes.add(...) and materials.add(...) every respawn tick, leaking a new asset per batch. Cache the handles once (e.g., in a resource populated at
   startup) and clone.
8. src/food.rs:153-155, 168-170 — same asset-leak pattern. Each spawn_food call creates a new Circle mesh + ColorMaterial. For 4 initial + many respawns this is wasteful. Share handles via a resource.
9. src/pheromone.rs:137-159 — diffusion clones two Vec<f32> every decay tick (GRID_W\*GRID_H = 36,864 f32 each). Use a double-buffer resource (owned Vecs swapped each tick) to avoid allocation. Also diffusion only applies to intensity, not direction
   components, so direction-weighted pheromones and intensity diverge after a few decay ticks.
10. src/pheromone.rs:103-162 — decay does not short-circuit walls. Decay runs over all GRID_W\*GRID_H cells including walls, even though walls never receive deposits. Minor cost but avoidable.
11. src/pheromone.rs:165-208 — texture update runs over full grid every frame it's dirty (36k iter) even though walls don't change. Consider writing walls once at setup and only updating pheromone cells, or using a smaller texture layout.
12. src/ant.rs:191 — handle_collision axis-aligned bounces are wrong inside the cave. pos.x < -half_w + 5.0 triggers PI - angle reflect, but an ant mid-cave hitting a wall doesn't respect X/Y orientation of that wall. The bounce is ad hoc; using the
    probe-fallback below is better. The X/Y boundary reflects specifically handle window edges and should remain only for those cases (already fine — just flagging that the ordering matters).
13. src/terrain.rs:258-272 — 1-cell-clearance fallback appends to open_cells that may already contain 2-cell-clearance entries. The outer if open_cells.is_empty() guards this, so it's actually fine — just confirm by reading.

Inefficiencies

14. src/ant.rs:224-229 — O(N_ants × N_food) scan every frame. For 2000 ants vs ~30 food entities, 60,000 distance checks per frame. Acceptable at current scale but would benefit from a spatial grid once food count grows, or just from the
    squared-distance fix above.
15. src/main.rs:73-81 — FoodPositions rebuilt every PreUpdate. Fine, but clear() + push could be positions.0 = food_query.iter().map(...).collect() once; or keep incremental if food changes are rare. Minor.
16. src/ant.rs:189 — from_entropy() per system-instance init. SmallRng::from_entropy is fine once; no issue.
17. src/pheromone.rs:187-207 — per-pixel branch on overlay.visible. Hoist the if overlay.visible check out of the inner loop (two code paths: fill transparent vs fill from grid).
18. src/pheromone.rs:192 — world_map.walls[grid_i] accessed per pixel even though decay already knows wall status. Same suggestion: separate wall-mask texture, or pre-split loops.
19. src/main.rs:263-275 — update_score_ui reallocates a new Text every tick ants change. Since Colony.active changes every few frames, this allocates a new Text instance constantly. Mutate text.0 string in place via text.sections / text.0 =
    format!(...) — or assign to a buffer. (Bevy 0.15 Text::new returns a new struct — just format into the existing text's content if possible.)
20. src/ant.rs:132-148 — 8-attempt RNG loop uses rand::random::<f32>(). Each call locks the thread-local RNG. Swap to the injected SmallRng.
21. src/ant.rs:165-176 — angle_diff uses while loops. Could be a single rem_euclid(2π) - π style. Micro-optimization.
22. src/terrain.rs:195-205 — center exclusion uses sqrt in inner loop. Compare squared: (dx*dx + dy*dy) < EXCL\*EXCL. Trivial win but clean.
23. src/main.rs:116 — HashSet<usize> for occupied cells. Replace with vec![false; GRID_W*GRID_H] or a BitVec — faster and sized-known.
24. src/ant.rs:318-325 — no system ordering between ant_behavior_system and ant_age_system. Behavior runs in parallel with age; age_system can despawn an ant whose behavior has already written deposits this frame — harmless today, but
    ant_respawn_system is in main's Update separately from AntPlugin additions — fragile coupling.
25. Mesh/material handle pattern pervasive — Bevy idiom is to insert these once into a resource (e.g., AntAssets { mesh, material }). Reduces allocations and keeps asset count bounded.

Style / Minor

26. src/ant.rs:1-2 — unused SeedableRng import? It's used via SmallRng::from_entropy only; verify.
27. src/main.rs:12 — use std::collections::HashSet — after fix #23 this can be removed.
28. src/pheromone.rs:116-134 — repeat block for home and food. Extract a decay_channel(intensity, dx, dy) helper.
29. src/terrain.rs:217-231 — BFS uses saturating_sub/min which still visits the boundary cell itself. Benign, but prefer explicit bounds check to avoid re-pushing the same index as neighbor.
30. Config magic numbers in code — 0.001, 0.2 + 0.8 _, 0.6 / 0.4 blend, 0.15 base-noise scalar, probe_dist = r _ 3.0, +5.0 boundary padding — consider promoting to config.rs constants.
31. No tests, no benchmarks. Given the parallel/ECS nature of the sim, even a smoke test ensuring ant_behavior_system runs without panic on a minimal World would catch regressions.

Top Priorities (if/when you implement)

1. Cache mesh/material handles in an AntAssets/FoodAssets resource (fixes #7, #8, #25 — biggest actual perf gain at scale).
2. Double-buffer pheromone diffusion to eliminate per-decay-tick Vec clone (#9).
3. Squared-distance comparisons everywhere (#2, #4, #22).
4. Pass SmallRng into handle_collision (#6, #20).
5. Diffuse direction channels alongside intensity — currently they drift apart (#9).
