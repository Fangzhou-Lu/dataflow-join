use std::rc::Rc;
use std::marker::PhantomData;

use typedrw::TypedMemoryMap;
use PrefixExtender;

pub trait GraphExtenderExt<G: GraphTrait> {
    fn extend_using<P,L: Fn(&P)->u64+'static>(&self, route: L) -> Rc<GraphExtender<G,P,L>>;
}

impl<G: GraphTrait> GraphExtenderExt<G> for Rc<G> {
    fn extend_using<P,L: Fn(&P)->u64+'static>(&self, logic: L) -> Rc<GraphExtender<G,P,L>> {
        Rc::new(GraphExtender {
            graph:  self.clone(),
            logic:  Rc::new(logic),
            phant:  PhantomData,
        })
    }
}

pub trait GraphTrait : 'static {
    type Target: Ord;
    fn nodes(&self) -> usize;
    fn edges(&self, node: usize) -> &[Self::Target];
}

pub struct GraphVector<E> {
    pub nodes: Vec<u64>,
    pub edges: Vec<E>,
}

impl<E: Ord+Send+'static> GraphTrait for GraphVector<E> {
    type Target = E;
    #[inline(always)]
    fn nodes(&self) -> usize {
        self.nodes.len()
    }
    #[inline(always)]
    fn edges(&self, node: usize) -> &[E] {
        if node + 1 < self.nodes.len() {
            let start = self.nodes[node] as usize;
            let limit = self.nodes[node+1] as usize;
            &self.edges[start..limit]
        }
        else { &[] }
    }
}

pub struct GraphMMap<E: Ord+Copy> {
    nodes: TypedMemoryMap<u64>,
    edges: TypedMemoryMap<E>,
}

impl<E: Ord+Copy> GraphMMap<E> {
    pub fn new(prefix: &str) -> GraphMMap<E> {
        GraphMMap {
            nodes: TypedMemoryMap::new(format!("{}.offsets", prefix)),
            edges: TypedMemoryMap::new(format!("{}.targets", prefix)),
        }
    }
}

impl<E: Ord+Copy+Send+'static> GraphTrait for GraphMMap<E> {
    type Target = E;
    #[inline(always)]
    fn nodes(&self) -> usize {
        self.nodes[..].len()
    }
    #[inline(always)]
    fn edges(&self, node: usize) -> &[E] {
        let nodes = &self.nodes[..];
        if node + 1 < nodes.len() {
            let start = nodes[node] as usize;
            let limit = nodes[node+1] as usize;
            &self.edges[..][start..limit]
        }
        else { &[] }
    }
}

pub struct GraphExtender<G: GraphTrait, P, L: Fn(&P)->u64> {
    graph: Rc<G>,
    logic: Rc<L>,
    phant: PhantomData<P>,
}

impl<G: GraphTrait, P, L: Fn(&P)->u64+'static> PrefixExtender for GraphExtender<G, P, L>
where <G as GraphTrait>::Target : Clone {
    type Prefix = P;
    type Extension = G::Target;

    type RoutingFunction = L;
    fn logic(&self) -> Rc<L> { self.logic.clone() }

    fn count(&self, prefix: &P) -> u64 {
        let node = (*self.logic)(prefix) as usize;
        self.graph.edges(node).len() as u64
    }

    fn propose(&self, prefix: &P, list: &mut Vec<G::Target>) {
        let node = (*self.logic)(prefix) as usize;
        *list = self.graph.edges(node).to_vec();
//        list.extend(self.graph.edges(node).iter().cloned());
    }

    fn intersect(&self, prefix: &P, list: &mut Vec<G::Target>) {
        let node = (*self.logic)(prefix) as usize;
        let mut slice = self.graph.edges(node);

        if list.len() < slice.len() / 4 {
            list.retain(|value| {
                slice = gallop(slice, value);
                slice.len() > 0 && &slice[0] == value
            });
        }
        else {
            list.retain(move |value| {
                while slice.len() > 0 && &slice[0] < value { slice = &slice[1..]; }
                slice.len() > 0 && &slice[0] == value
            });
        }
    }
}

// intended to advance slice to start at the first element >= value.

#[inline(always)]
pub fn gallop<'a, T: Ord>(mut slice: &'a [T], value: &T) -> &'a [T] {
    // if empty slice, or already >= element, return
    if slice.len() > 0 && &slice[0] < value {
        let mut step = 1;
        while step < slice.len() && &slice[step] < value {
            slice = &slice[step..];
            step = step << 1;
        }

        step = step >> 1;
        while step > 0 {
            if step < slice.len() && &slice[step] < value {
                slice = &slice[step..];
            }
            step = step >> 1;
        }

        slice = &slice[1..]; // advance one, as we always stayed < value
    }

    return slice;
}
