import {
  MAX_ROUTING_OBSTACLES,
  MAX_ROUTING_VISITS,
  ROUTING_CLEARANCE_REM,
  ROUTING_MARGIN_REM,
  routeEdgeAroundObstacles,
  routingSearchWithinBudget,
  type RoutingObstacle,
} from "./edge-routing";
import type { GraphGeometryDelta } from "./retained-presentation";
import { SpatialIndex, type SpatialBounds } from "./spatial-index";
import type {
  GraphEdge,
  GraphNodePosition,
  GraphPresentationDocument,
  GraphSocketDirection,
  GraphSocketRef,
} from "./types";
import {
  boundsIntersect,
  cubicWireGeometry,
  expandedBounds,
  loopWireGeometry,
  pointsBounds,
  scaledWirePath,
  unionBounds,
  type GraphWireGeometry,
} from "./wire-geometry";

const CACHE_LIMIT = 1024;
const EDGE_CELL_SIZE = 64;
const JOBS_PER_FRAME = 4;
const VISITS_PER_FRAME = MAX_ROUTING_VISITS * 2;

interface EdgeGeometry {
  index: number;
  start: GraphNodePosition;
  end: GraphNodePosition;
  routeStart: GraphNodePosition;
  routeEnd: GraphNodePosition;
  search: SpatialBounds;
  fallback: GraphWireGeometry;
}

interface CachedRoute {
  geometry: GraphWireGeometry;
  scale: number | null;
  path: string | null;
}

export interface GraphVisibleEdge {
  index: number;
  edge: GraphEdge;
}
export interface GraphRoutingWork {
  obstacleUpdates: number;
  edgeGeometryUpdates: number;
  invalidatedRoutes: number;
  routeJobs: number;
  searchVisits: number;
  smoothingChecks: number;
  obstacleBudgetFallbacks: number;
  workSlices: number;
  maxSliceJobs: number;
  maxSliceVisits: number;
  maxSliceSmoothingChecks: number;
  cachedRoutes: number;
  pendingRoutes: number;
}

type SocketPoint = (
  reference: GraphSocketRef,
  direction: GraphSocketDirection,
) => GraphNodePosition | null;
const sameEdge = (a: GraphEdge | undefined, b: GraphEdge): boolean =>
  Boolean(
    a &&
    a.id === b.id &&
    a.from.nodeId === b.from.nodeId &&
    a.from.socketId === b.from.socketId &&
    a.to.nodeId === b.to.nodeId &&
    a.to.socketId === b.to.socketId,
  );
const sameBounds = (a: SpatialBounds | undefined, b: SpatialBounds): boolean =>
  Boolean(
    a &&
    a.left === b.left &&
    a.top === b.top &&
    a.right === b.right &&
    a.bottom === b.bottom,
  );

/** World-unit geometry, bounded route jobs and conservative path-envelope visibility. */
export class RetainedEdgePresentation {
  #source: object | null = null;
  #topology = -1;
  #edges: readonly GraphEdge[] = [];
  readonly #nodes = new Map<string, SpatialBounds>();
  #obstacles = new SpatialIndex<RoutingObstacle>();
  #edgeBounds = new SpatialIndex<EdgeGeometry>(EDGE_CELL_SIZE);
  #dependencies = new SpatialIndex<number>();
  readonly #geometry = new Map<number, EdgeGeometry>();
  readonly #incident = new Map<string, Set<number>>();
  readonly #cache = new Map<number, CachedRoute>();
  readonly #pending = new Set<number>();
  #admitted = new Set<number>();
  #routing = false;
  readonly #work = {
    obstacleUpdates: 0,
    edgeGeometryUpdates: 0,
    invalidatedRoutes: 0,
    routeJobs: 0,
    searchVisits: 0,
    smoothingChecks: 0,
    obstacleBudgetFallbacks: 0,
    workSlices: 0,
    maxSliceJobs: 0,
    maxSliceVisits: 0,
    maxSliceSmoothingChecks: 0,
  };

  get statistics(): Readonly<GraphRoutingWork> {
    return {
      ...this.#work,
      cachedRoutes: this.#cache.size,
      pendingRoutes: this.#pending.size,
    };
  }
  get pending(): boolean {
    return this.#pending.size > 0;
  }

  synchronize(
    document: GraphPresentationDocument,
    delta: GraphGeometryDelta,
    socketPoint: SocketPoint,
  ): void {
    const replaced = document.source !== this.#source || delta.reset;
    if (replaced) {
      this.#nodes.clear();
      this.#geometry.clear();
      this.#incident.clear();
      this.#cache.clear();
      this.#pending.clear();
      this.#admitted.clear();
      this.#obstacles = new SpatialIndex();
      this.#edgeBounds = new SpatialIndex(EDGE_CELL_SIZE);
      this.#dependencies = new SpatialIndex();
      this.#edges = [];
      this.#topology = -1;
      this.#source = document.source;
    }
    const movedEdges = new Set<number>();
    const dirtyNodes = new Set<string>();
    for (const id of delta.removed) {
      dirtyNodes.add(id);
      const previous = this.#nodes.get(id);
      if (previous)
        this.#invalidateObstacles(
          expandedBounds(previous, ROUTING_CLEARANCE_REM),
        );
      this.#nodes.delete(id);
      this.#obstacles.remove(id);
      this.#work.obstacleUpdates += 1;
    }
    for (const { id, bounds } of delta.updated) {
      dirtyNodes.add(id);
      const previous = this.#nodes.get(id);
      if (sameBounds(previous, bounds)) continue;
      if (previous)
        this.#invalidateObstacles(
          expandedBounds(previous, ROUTING_CLEARANCE_REM),
        );
      const obstacle = expandedBounds(bounds, ROUTING_CLEARANCE_REM);
      this.#invalidateObstacles(obstacle);
      this.#nodes.set(id, bounds);
      if (previous) this.#obstacles.update(id, obstacle, obstacle);
      else this.#obstacles.insert(id, obstacle, obstacle);
      this.#work.obstacleUpdates += 1;
    }
    for (const id of dirtyNodes)
      for (const index of this.#incident.get(id) ?? []) movedEdges.add(index);
    if (document.revision.topology !== this.#topology) {
      for (
        let index = 0;
        index < Math.max(this.#edges.length, document.edges.length);
        index += 1
      ) {
        const previous = this.#edges[index],
          next = document.edges[index];
        if (next && sameEdge(previous, next)) continue;
        if (previous) {
          for (const id of [previous.from.nodeId, previous.to.nodeId]) {
            const entries = this.#incident.get(id);
            entries?.delete(index);
            if (entries?.size === 0) this.#incident.delete(id);
          }
        }
        if (next) {
          for (const id of [next.from.nodeId, next.to.nodeId]) {
            const entries = this.#incident.get(id) ?? new Set();
            entries.add(index);
            this.#incident.set(id, entries);
          }
        }
        movedEdges.add(index);
      }
      this.#topology = document.revision.topology;
    }
    this.#edges = document.edges;
    for (const index of movedEdges) {
      this.#invalidate(index);
      const edge = this.#edges[index];
      const start = edge && socketPoint(edge.from, "output"),
        end = edge && socketPoint(edge.to, "input");
      const source = edge && this.#nodes.get(edge.from.nodeId),
        target = edge && this.#nodes.get(edge.to.nodeId);
      if (!edge || !start || !end || !source || !target) {
        this.#geometry.delete(index);
        this.#edgeBounds.remove(String(index));
        continue;
      }
      const routeStart = {
        x: Math.ceil(source.right + ROUTING_CLEARANCE_REM) + 1,
        y: start.y,
      };
      const routeEnd = {
        x: Math.floor(target.left - ROUTING_CLEARANCE_REM) - 1,
        y: end.y,
      };
      const search = expandedBounds(
        pointsBounds([start, end, routeStart, routeEnd]),
        ROUTING_MARGIN_REM,
      );
      const fallback =
        edge.from.nodeId === edge.to.nodeId
          ? loopWireGeometry(start, end, source)
          : cubicWireGeometry(start, end);
      const geometry = {
        index,
        start,
        end,
        routeStart,
        routeEnd,
        search,
        fallback,
      };
      const bounds = unionBounds(search, fallback.bounds);
      if (this.#geometry.has(index))
        this.#edgeBounds.update(String(index), bounds, geometry);
      else this.#edgeBounds.insert(String(index), bounds, geometry);
      this.#geometry.set(index, geometry);
      this.#work.edgeGeometryUpdates += 1;
    }
  }

  visible(
    bounds: SpatialBounds,
    routing: boolean,
    preview: boolean,
  ): GraphVisibleEdge[] {
    this.#routing = routing;
    const candidates = this.#edgeBounds
      .query(bounds)
      .sort((a, b) => a.index - b.index);
    this.#admitted = new Set(
      routing
        ? candidates.slice(0, CACHE_LIMIT).map((entry) => entry.index)
        : [],
    );
    for (const index of this.#pending)
      if (preview || !this.#admitted.has(index)) this.#pending.delete(index);
    const result: GraphVisibleEdge[] = [];
    for (const candidate of candidates) {
      const cached = this.#admitted.has(candidate.index)
        ? this.#cache.get(candidate.index)
        : undefined;
      if (routing && !preview && this.#admitted.has(candidate.index) && !cached)
        this.#pending.add(candidate.index);
      const geometry = routing && cached ? cached.geometry : candidate.fallback;
      if (boundsIntersect(geometry.bounds, bounds))
        result.push({
          index: candidate.index,
          edge: this.#edges[candidate.index],
        });
    }
    return result;
  }

  path(index: number, scale: number): string | null {
    const entry = this.#geometry.get(index);
    if (!entry) return null;
    const cached =
      this.#routing && this.#admitted.has(index)
        ? this.#cache.get(index)
        : undefined;
    if (!cached) return scaledWirePath(entry.fallback, scale);
    if (cached.scale !== scale) {
      cached.path = scaledWirePath(cached.geometry, scale);
      cached.scale = scale;
    }
    return cached.path;
  }

  processPending(): void {
    let remainingVisits = VISITS_PER_FRAME,
      jobs = 0;
    let smoothingChecks = 0;
    for (const index of this.#pending) {
      if (jobs >= JOBS_PER_FRAME || remainingVisits < MAX_ROUTING_VISITS) break;
      this.#pending.delete(index);
      const entry = this.#geometry.get(index);
      if (!entry || !this.#admitted.has(index)) continue;
      const independentFallback = !routingSearchWithinBudget(
        entry.routeStart,
        entry.routeEnd,
        1,
        MAX_ROUTING_VISITS,
      );
      const obstacles = independentFallback
        ? { values: [], complete: true }
        : this.#obstacles.queryLimited(entry.search, MAX_ROUTING_OBSTACLES * 4);
      const budget = {
        remainingVisits: MAX_ROUTING_VISITS,
        remainingChecks: 512,
      };
      const limited =
        !obstacles.complete || obstacles.values.length > MAX_ROUTING_OBSTACLES;
      if (limited) this.#work.obstacleBudgetFallbacks += 1;
      const geometry =
        (!limited &&
          !independentFallback &&
          routeEdgeAroundObstacles(
            entry.start,
            entry.end,
            entry.routeStart,
            entry.routeEnd,
            obstacles.values,
            1,
            budget,
          )) ||
        entry.fallback;
      const visits = MAX_ROUTING_VISITS - budget.remainingVisits;
      remainingVisits -= visits;
      jobs += 1;
      this.#work.routeJobs += 1;
      this.#work.searchVisits += visits;
      this.#work.smoothingChecks += 512 - budget.remainingChecks;
      smoothingChecks += 512 - budget.remainingChecks;
      while (this.#cache.size >= CACHE_LIMIT) {
        const evict =
          [...this.#cache.keys()].find((key) => !this.#admitted.has(key)) ??
          this.#cache.keys().next().value!;
        this.#cache.delete(evict);
        this.#dependencies.remove(String(evict));
      }
      this.#cache.set(index, { geometry, scale: null, path: null });
      if (!independentFallback)
        this.#dependencies.insert(String(index), entry.search, index);
    }
    if (jobs > 0) {
      this.#work.workSlices += 1;
      this.#work.maxSliceJobs = Math.max(this.#work.maxSliceJobs, jobs);
      this.#work.maxSliceVisits = Math.max(
        this.#work.maxSliceVisits,
        VISITS_PER_FRAME - remainingVisits,
      );
      this.#work.maxSliceSmoothingChecks = Math.max(
        this.#work.maxSliceSmoothingChecks,
        smoothingChecks,
      );
    }
  }

  #invalidateObstacles(bounds: SpatialBounds): void {
    for (const index of this.#dependencies.query(bounds))
      this.#invalidate(index);
  }
  #invalidate(index: number): void {
    if (this.#cache.delete(index)) this.#work.invalidatedRoutes += 1;
    this.#dependencies.remove(String(index));
    this.#pending.delete(index);
  }
}
