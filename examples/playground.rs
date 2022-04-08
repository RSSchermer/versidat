#![feature(generic_associated_types)]

use std::ops::Deref;
use viemo::memo::{CellIterMemo, Memo, OptionCellMemo, OptionNodeMemo, OwnedMemo};
use viemo::watcher::Watcher;

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
        node_elements: Vec<VersionedCell<'store, NodeElement<'store>>>,
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
        node_elements: vec![],
    });

    let mut cell_memo = CellMemo::new(&store, |root, _| &root.element);
    let mut option_cell_memo = OptionCellMemo::new(&store, |root, _| root.elements.get(0));
    let mut node_memo = NodeMemo::<NodeElementTC, _, _>::new(&store, |root, _| &root.node_element);
    let mut option_node_memo =
        OptionNodeMemo::<NodeElementTC, _, _>::new(&store, |root, _| root.node_elements.get(0));
    let mut owned_memo = OwnedMemo::new(&store, |root, cx| root.element.deref(cx).a);

    // let mut iter_memo = CellIterMemo::new(&store, |root: &MyRoot, cx| root.elements.iter());
    //
    // let mut watcher2 = Watcher2::new(&store, cell_memo, node_memo, |(cell, node), cx| {
    //     println!("{} {}", cell.deref(cx).a, node.deref(cx).b);
    // });

    //
    // let mut watcher = Watcher2::new(&store, cell_memo, node_memo);

    // let render = async move {
    //     while let Some(view) = watcher.next().await {
    //         view.with(|(cell, node), cx| {
    //             println!("{} {}", cell.deref(cx).a, node.deref(cx).b);
    //         })
    //     }
    // };
    // let mut on_update = store.on_update();
    //
    // let render = async move {
    //     while let Some(_) = on_update.next().await {
    //         store.with(|root, cx| {
    //             let cell = cell_memo.refresh(root, cx);
    //             let node = node_memo.refresh(root, cx);
    //
    //             if cell.is_changed() || node.is_changed() {
    //                 println!("{} {}", cell.deref(cx).a, node.deref(cx).b);
    //             }
    //         })
    //     }
    // };
}
