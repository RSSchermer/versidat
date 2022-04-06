mod memo;
pub use self::memo::*;

mod selector;
pub use self::selector::*;

pub mod cell;
pub mod node;
pub mod owned;

fn test() {
    use futures::StreamExt;

    use crate::gen_type_constructor;
    use crate::versioned_cell::VersionedCell;
    use crate::store::Store;
    use crate::memo::cell::CellMemo;
    use crate::memo::node::NodeMemo;
    use crate::watcher::Watcher2;

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
    }

    gen_type_constructor!(NodeElement, NodeElementTC);

    type MyStore = Store<MyRootTC>;

    let store = MyStore::initialize(|cx| MyRoot {
        element: VersionedCell::new(cx, Element { a: 0 }),
        node_element: VersionedCell::new(
            cx,
            NodeElement {
                element: VersionedCell::new(cx, Element { a: 1 }),
            },
        ),
        elements: vec![],
    });

    let cell_memo = CellMemo::new(&store, |root, cx| &root.element);
    let node_memo = NodeMemo::<NodeElementTC, _, _>::new(&store, |root, cx| &root.node_element);

    let mut watcher = Watcher2::new(&store, cell_memo, node_memo);

    let block = async move {
        while let Some(view) = watcher.next().await {
            view.with(|(cell, node), cx| {
                println!("{} {}", cell.deref(cx).a, node.deref(cx).element.deref(cx).a);
            })
        }
    };
}
