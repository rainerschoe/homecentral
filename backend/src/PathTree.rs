#[derive(PartialEq, Clone)]
pub enum PathElement
{
    Root,
    Name(String),
    Wildcard
}

impl PathElement
{
    fn matches(self: &Self, other: &Self) -> bool
    {
        use PathElement::*;
        match self 
        {
            Root => self == other,
            Name(_) => self == other || matches!(other, Wildcard),
            Wildcard => true
        }
    }
}

pub struct PathTree<T>
{
    element: PathElement,
    payloads: Vec<T>,
    childs: Vec<PathTree<T>>,
}
// /root/
//       EG
//          sub1
//          sub2
//          sub3
//          Schlafzi
//              sub1
//              sub2
//       KG
//          Schlafzi
//              sub5
//       OG
//       *
//            Schlafzi
//                sub3
//                sub4

// publish /home/EG/Schlafzi
//     -> sub1, sub2, sub3, sub4
// publish /home/*/Schlafzi
//     -> sub1, sub2, sub5, sub3, sub4
impl<T> PathTree<T>
{
    pub fn new() -> Self
    {
        use PathElement::*;
        PathTree{element: Root, payloads: Vec::new(), childs: Vec::new()}
    }
    pub fn add_payload(self: &mut Self, path: &[PathElement], payload: T)
    {
        use PathElement::*;
        if path.is_empty()
        {
            return;
        }

        if path[0] == Root
        {
            if self.element != Root
            {
                panic!("Trying to add root element to tree. NOTE: Only Absolute paths may have a root element. And this is only allowed as first element.");
            }
            // we can only append childs, 
            return self.add_payload(&path[1..], payload);
        }

        let existing_child =
            self.childs.iter_mut().find(|x|
                {x.element == path[0]}
            );
        let child: &mut PathTree<T>  = match existing_child
        {
            Some( child) => child,
            None => 
            {
                self.childs.push(
                    PathTree{
                        element: path[0].clone(),
                        payloads: Vec::new(),
                        childs: Vec::new()
                    }
                    );
                self.childs.last_mut().unwrap()
            }
        };

        if path.len() == 1
        {
            child.payloads.push(payload);
            return;
        }

        return child.add_payload(&path[1..], payload)
    }

    pub fn get_payloads(self: &Self, path: &[PathElement]) -> Vec<&T>
    {
        if path.is_empty()
        {
            return Vec::new();
        }

        // determine if this level in path matches the current level in the tree:
        let matches = self.element.matches(&path[0]);
        if !matches
        {
            return Vec::new();
        }

        // now handle childs:
        if path.len() == 1
        {
            // we are at the end of the path (not necessary the end of the tree) and have a match :) YAY!!
            // return all payloads
            return Vec::from_iter(self.payloads.iter());
        }

        let mut result = Vec::new();
        for sub_tree in self.childs.iter()
        {
            result.append(&mut sub_tree.get_payloads(&path[1..]));
        }
        return result;
    }
}

#[test]
fn add_subscriber()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&[Root, Name("test".into())], "data");
    assert!(tree.element == Root);
    assert!(tree.childs.len() == 1);
    assert!(tree.childs[0].element == Name("test".into()));
    assert!(tree.childs[0].payloads.len() == 1);
    assert!(tree.childs[0].payloads[0] == "data");
}

#[test]
fn add_2subscriber_same_path()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&[Root, Name("test".into())], "data");
    tree.add_payload(&[Root, Name("test".into())], "data2");
    assert!(tree.element == Root);
    assert!(tree.childs.len() == 1);
    assert!(tree.childs[0].element == Name("test".into()));
    assert!(tree.childs[0].payloads.len() == 2);
    assert!(tree.childs[0].payloads[0] == "data");
    assert!(tree.childs[0].payloads[1] == "data2");
}

#[test]
fn add_2subscriber_different_path()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&[Root, Name("test".into())], "data");
    tree.add_payload(&[Root, Name("test2".into())], "data2");
    assert!(tree.element == Root);
    assert!(tree.childs.len() == 2);
    assert!(tree.childs[0].element == Name("test".into()));
    assert!(tree.childs[0].payloads.len() == 1);
    assert!(tree.childs[0].payloads[0] == "data");
    assert!(tree.childs[1].element == Name("test2".into()));
    assert!(tree.childs[1].payloads.len() == 1);
    assert!(tree.childs[1].payloads[0] == "data2");
}

#[test]
fn add_2subscriber_different_deep_path()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&[Root, Name("l1".into()), Name("l12".into())], "data1");
    tree.add_payload(&[Root, Name("l2".into()), Name("l22".into())], "data2");
    assert!(tree.element == Root);
    assert!(tree.childs.len() == 2);
    assert!(tree.childs[0].element == Name("l1".into()));
    assert!(tree.childs[0].payloads.len() == 0);
    assert!(tree.childs[1].element == Name("l2".into()));
    assert!(tree.childs[1].payloads.len() == 0);

    assert!(tree.childs[0].childs.len() == 1);
    assert!(tree.childs[0].childs[0].element == Name("l12".into()));
    assert!(tree.childs[0].childs[0].payloads.len() == 1);
    assert!(tree.childs[0].childs[0].payloads[0] == "data1");

    assert!(tree.childs[1].childs.len() == 1);
    assert!(tree.childs[1].childs[0].element == Name("l22".into()));
    assert!(tree.childs[1].childs[0].payloads.len() == 1);
    assert!(tree.childs[1].childs[0].payloads[0] == "data2");

}