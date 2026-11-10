# Cached Component Energy Grids

To prevent the tick degradation caused by per-tick breadth-first search (BFS) cable traversals in original Java HBM, NBM_R-Native maintains energy network connectivity as cached disjoint components. Grid topology updates only on block placement and destruction events, allowing level_tick_pre energy distribution to run in O(1) graph time without pathfinding overhead.
