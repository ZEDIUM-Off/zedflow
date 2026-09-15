import { AvoidLib } from 'libavoid-js'
import wasmUrl from '@libavoid-assets/libavoid.wasm?url'

export interface Point { x: number; y: number }
export interface RoutingPort extends Point { id: string; side: 'left' | 'right' | 'top' | 'bottom' }
export interface RoutingNode extends Point { id: string; width: number; height: number; ports: RoutingPort[] }
export interface RoutingEdge { id: string; source: string; target: string; sourcePort: string; targetPort: string }
export interface RoutedEdge { points: Point[]; path: string; label: Point }
interface NativeShape {}
interface NativeConnector { displayRoute(): {size(): number; get_ps(index: number): Point}; setRoutingType(value: number): void }
interface NativeRouter { processTransaction(): void; moveShape(shape: NativeShape, dx: number, dy: number): void; setRoutingParameter(key: number, value: number): void }
interface AvoidApi {
  Router: new(flags: number) => NativeRouter
  Point: new(x: number, y: number) => Point
  Rectangle: new(start: Point, end: Point) => object
  ShapeRef: new(router: NativeRouter, rectangle: object) => NativeShape
  ShapeConnectionPin: new(shape: NativeShape, id: number, x: number, y: number, proportional: boolean, insideOffset: number, direction: number) => {setExclusive(value: boolean): void}
  ConnEnd: new(shape: NativeShape, id: number) => object
  ConnRef: new(router: NativeRouter, source: object, target: object) => NativeConnector
  OrthogonalRouting: number; ConnDirLeft: number; ConnDirRight: number; ConnDirUp: number; ConnDirDown: number
  shapeBufferDistance: number; idealNudgingDistance: number; segmentPenalty: number
  destroy(value: unknown): void
}
let loading: Promise<AvoidApi> | undefined
export function loadRouter() {
  return loading ??= AvoidLib.load(wasmUrl).then(() => AvoidLib.getInstance() as unknown as AvoidApi).catch(error => { loading = undefined; throw error })
}

function routeData(points: Point[]): RoutedEdge {
  let label = points[0] || {x: 0, y: 0}, longest = 0
  for (let index = 1; index < points.length; index++) {
    const before = points[index - 1], next = points[index]
    const distance = Math.abs(next.x - before.x) + Math.abs(next.y - before.y)
    if (distance > longest) { longest = distance; label = {x: (before.x + next.x) / 2, y: (before.y + next.y) / 2} }
  }
  return {points, label, path: points.map((point, index) => `${index ? 'L' : 'M'} ${point.x} ${point.y}`).join(' ')}
}

/** Owns native objects for one visible graph. Geometry never enters the flow document. */
export class ObstacleRouter {
  private router?: NativeRouter
  private signature = ''
  private shapes = new Map<string, {shape: NativeShape; x: number; y: number; pins: Map<string, number>}>()
  private connectors = new Map<string, NativeConnector>()
  constructor(private avoid: AvoidApi) {}

  update(nodes: RoutingNode[], edges: RoutingEdge[]): Record<string, RoutedEdge> {
    const signature = JSON.stringify([nodes.map(({id, width, height, ports}) => ({id, width, height, ports})), edges])
    if (!this.router || this.signature !== signature) {
      this.destroy()
      this.signature = signature
      const A = this.avoid, router = this.router = new A.Router(A.OrthogonalRouting)
      router.setRoutingParameter(A.shapeBufferDistance, 12)
      router.setRoutingParameter(A.idealNudgingDistance, 8)
      router.setRoutingParameter(A.segmentPenalty, 20)
      for (const node of nodes) {
        const start = new A.Point(node.x, node.y), end = new A.Point(node.x + node.width, node.y + node.height)
        const rectangle = new A.Rectangle(start, end), shape = new A.ShapeRef(router, rectangle)
        A.destroy(rectangle); A.destroy(start); A.destroy(end)
        const pins = new Map<string, number>()
        for (const [index, port] of node.ports.entries()) {
          const direction = {left: A.ConnDirLeft, right: A.ConnDirRight, top: A.ConnDirUp, bottom: A.ConnDirDown}[port.side]
          const pin = new A.ShapeConnectionPin(shape, index + 1, port.x / node.width, port.y / node.height, true, 0, direction)
          pin.setExclusive(false)
          pins.set(port.id, index + 1)
        }
        this.shapes.set(node.id, {shape, x: node.x, y: node.y, pins})
      }
      for (const edge of edges) {
        const source = this.shapes.get(edge.source), target = this.shapes.get(edge.target)
        const sourcePin = source?.pins.get(edge.sourcePort), targetPin = target?.pins.get(edge.targetPort)
        if (!source || !target || sourcePin === undefined || targetPin === undefined) continue
        const from = new A.ConnEnd(source.shape, sourcePin), to = new A.ConnEnd(target.shape, targetPin)
        const connector = new A.ConnRef(router, from, to)
        connector.setRoutingType(A.OrthogonalRouting)
        A.destroy(from); A.destroy(to)
        this.connectors.set(edge.id, connector)
      }
    } else {
      for (const node of nodes) {
        const value = this.shapes.get(node.id)!
        if (value.x !== node.x || value.y !== node.y) {
          this.router.moveShape(value.shape, node.x - value.x, node.y - value.y)
          value.x = node.x; value.y = node.y
        }
      }
    }
    this.router.processTransaction()
    const result: Record<string, RoutedEdge> = {}
    for (const [id, connector] of this.connectors) {
      const line = connector.displayRoute(), points: Point[] = []
      for (let index = 0; index < line.size(); index++) {
        const point = line.get_ps(index)
        points.push({x: point.x, y: point.y})
      }
      if (points.length >= 2) result[id] = routeData(points)
    }
    return result
  }

  destroy() {
    if (this.router) this.avoid.destroy(this.router)
    this.router = undefined; this.shapes.clear(); this.connectors.clear(); this.signature = ''
  }
}
