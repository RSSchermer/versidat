#![feature(generic_associated_types)]

fn main() {
    use futures::StreamExt;

    use viemo::gen_type_constructor;
    use viemo::memo::{CellMemo, NodeMemo};
    use viemo::store::Store;
    use viemo::versioned_cell::VersionedCell;
    use viemo::watcher::Watcher2;

    struct MyRoot<'store> {
        element: VersionedCell<'store, Element>,
        node_element: VersionedCell<'store, NodeElement<'store>>,
        elements: Vec<VersionedCell<'store, Element>>,
    }

    gen_type_constructor!(MyRoot, MyRootTC);

    struct Element {
        a: u32,
    }

    struct NodeElement<'store> {
        element: VersionedCell<'store, Element>,
        b: u32,
    }

    gen_type_constructor!(NodeElement, NodeElementTC);

    type MyStore = Store<MyRootTC>;

    let store = MyStore::initialize(|cx| MyRoot {
        element: VersionedCell::new(cx, Element { a: 0 }),
        node_element: VersionedCell::new(
            cx,
            NodeElement {
                element: VersionedCell::new(cx, Element { a: 1 }),
                b: 0,
            },
        ),
        elements: vec![],
    });

    let cell_memo = CellMemo::new(&store, |root, cx| &root.element);
    let node_memo = NodeMemo::<NodeElementTC, _, _>::new(&store, |root, cx| &root.node_element);

    let mut watcher = Watcher2::new(&store, cell_memo, node_memo);

    let render = async move {
        while let Some(view) = watcher.next().await {
            view.with(|(cell, node), cx| {
                println!("{} {}", cell.deref(cx).a, node.deref(cx).b);
            })
        }
    };
}
