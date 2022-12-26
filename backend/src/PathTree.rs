use std::{sync::Arc, iter::TrustedLen};

#[derive(PartialEq, Clone)]
pub enum PathElement
{
    Root,
    Name(String),
    Wildcard((usize, usize)) // (Min, Max) : Number of consumed nodes are any between Min and Max inclusive.
}

fn consume_wildcard(wildcard: &mut (usize, usize)) -> bool
{
    if(wildcard.0 > 0)
    {
        wildcard.0 -= 1;
    }
    else if (wildcard.1 > 0){
        wildcard.1 -= 1;
    }
    else {
        return true;
    }
    if (wildcard.0 == 0) && (wildcard.1 == 0)
    {
        return true;
    }
    else
    {
        return false;
    }
}

use PathElement::*;

impl PathElement
{
    fn matches(self: &mut Self, other: &mut Self) -> bool
    {
        use PathElement::*;
        match self 
        {
            Root => self == other || matches!(other, Wildcard),
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
        struct Job<'a, T>
        {
            path: &'a [PathElement],
            path_wildcard_override: Option<(usize,usize)>,

            tree: &'a PathTree<T>,
            tree_wildcard_override: Option<(usize,usize)>,
        }

        let initial_job = Job{
                                    path: path,
                                    path_wildcard_override: None,
                                    tree: self,
                                    tree_wildcard_override: None
                                    };

        let mut jobs = Vec::new();
        jobs.push(initial_job);

        let mut result = Vec::new();
        'jobloop:
        loop {
            if jobs.is_empty()
            {
                break 'jobloop;
            }
            let job = jobs.pop().unwrap();
            let tree = job.tree;
            let path = job.path;
            let tree_wildcard_override = job.tree_wildcard_override;
            let path_wildcard_override = job.path_wildcard_override;

            if path.is_empty()
            {
                result.extend_from_slice(&tree.payloads[..]);
                continue 'jobloop;
            }

            let tree_node = tree.element.clone();
            let path_node = path[0].clone();
            match tree_node
            {
                Name(tree_node_name) =>
                {
                    match path_node
                    {
                        // Name + Name
                        Name(path_node_name) =>
                        {
                            // match -> add all childs to job list
                            if tree_node_name == path_node_name
                            {
                                for child in self.childs.iter()
                                {
                                    let job = Job{
                                        path: path[1..],
                                        path_wildcard_override: None,
                                        tree: child,
                                        tree_wildcard_override: None
                                        };
                                    jobs.push(job);
                                }
                            }
                            else
                            {
                                // no match -> ignore this arm
                                continue 'jobloop;
                            }
                        },
                        // tree:Name + path:Wildcard
                        Wildcard(path_wildcard) =>
                        {
                            let mut path_wildcard = match job.path_wildcard_override
                            {
                                Some(wc_override) => wc_override,
                                None => path_wildcard
                            };
                            
                            if path_wildcard.0 == 0 and path_wildcard.1 == 0
                            {
                                // invalid wildcard
                                // remove it and contine
                                let job = Job{
                                    path: &path[1..], // full path, as WC might not be fully consumed
                                    path_wildcard_override: None,
                                    tree: &self,
                                    tree_wildcard_override: None
                                    };
                                jobs.push(job);
                                continue 'jobloop;
                            }

                            // When minimums is 0, we also have the choice to NOT consume and skip the wildcard
                            if path_wildcard.0 == 0
                            {
                                // remove it
                                let job = Job{
                                    path: &path[1..], // full path, as WC might not be fully consumed
                                    path_wildcard_override: None,
                                    tree: &self,
                                    tree_wildcard_override: None
                                    };
                                jobs.push(job);
                            }

                            // consuming is always an option:
                            // add all childs after consuming one from the wildcard
                            for child in self.childs.iter()
                            {
                                let mut new_path_wildcard = path_wildcard.clone();
                                consume_wildcard(new_path_wildcard);
                                let job = Job{
                                    path: &path[..], // full path, as WC might not be fully consumed
                                    path_wildcard_override: new_path_wildcard,
                                    tree: child,
                                    tree_wildcard_override: None
                                    };
                                jobs.push(job);
                            }

                            
                        }
                    }
                },
                Root =>
                {

                },
                Wildcard(tree_wildcard) => 
                {
                    // tree node is a wildcard => retrieve and override if required:
                    let mut tree_wildcard = match job.tree_wildcard_override
                    {
                        Some(wc_override) => wc_override,
                        None => tree_wildcard
                    }

                    match path_node 
                    {
                        // tree_node: Wildcard, path_node: Name
                        Name(path_node_name) =>
                        {
                            if tree_wildcard.0 == 0 and tree_wildcard.1 == 0
                            {
                                // invalid wildcard
                                // remove it and contine
                                for child in self.child.iter()
                                {
                                    let job = Job{
                                        path: &path[..], // full path, as WC might not be fully consumed
                                        path_wildcard_override: None,
                                        tree: child,
                                        tree_wildcard_override: None
                                        };
                                    jobs.push(job);
                                }
                                continue 'jobloop;
                            }

                            // no minumums required, so we might also skip wildcard here:
                            if tree_wildcard.0 == 0
                            {
                                for child in self.child.iter()
                                {
                                    let job = Job{
                                        path: &path[..], // full path, as WC might not be fully consumed
                                        path_wildcard_override: None,
                                        tree: child,
                                        tree_wildcard_override: None
                                        };
                                    jobs.push(job);
                                }
                            }

                            // consuming is always an option for valid wildcards:
                            for child in self.childs.iter()
                            {
                                let mut new_tree_wildcard = tree_wildcard.clone();
                                consume_wildcard(&new_tree_wildcard);
                                let job = Job{
                                    path: path[1..],
                                    path_wildcard_override: None,
                                    tree: child,
                                    tree_wildcard_override: Some(new_tree_wildcard)
                                    };
                                jobs.push(job);
                            }
                        }

                        // tree_node: Wildcard, path_node: Wildcard (the tricky case)
                        Wildcard(path_wildcard) =>
                        {
                            let mut path_wildcard = match job.path_wildcard_override
                            {
                                Some(wc_override) => wc_override,
                                None => path_wildcard
                            };

                            // we reduce this scenario down to a set of single wildcard scenarios by recursively removing/consuming from one wildcard:

                            if tree_wildcard.0 == 0 and tree_wildcard.1 == 0
                            {
                                // invalid tree wildcard
                                // remove it and contine
                                for child in self.child.iter()
                                {
                                    let job = Job{
                                        path: &path[..], // full path, as WC might not be fully consumed
                                        path_wildcard_override: None,
                                        tree: child,
                                        tree_wildcard_override: None
                                        };
                                    jobs.push(job);
                                }
                                continue 'jobloop;
                            }

                            if path_wildcard.0 == 0 and path_wildcard.1 == 0
                            {
                                // invalid path wildcard
                                // remove it and contine
                                let job = Job{
                                    path: &path[1..], // full path, as WC might not be fully consumed
                                    path_wildcard_override: None,
                                    tree: self,
                                    tree_wildcard_override: None
                                    };
                                jobs.push(job);
                                continue 'jobloop;
                            }


                            // no minumums required, so we might also skip wildcard here:
                            if tree_wildcard.0 == 0
                            {
                                for child in self.child.iter()
                                {
                                    let job = Job{
                                        path: &path[..], // full path, as WC might not be fully consumed
                                        path_wildcard_override: job.path_wildcard_override,
                                        tree: child,
                                        tree_wildcard_override: None
                                        };
                                    jobs.push(job);
                                }
                            }

                            // consuming is always an option for valid wildcards:
                            for child in self.childs.iter()
                            {
                                let mut new_tree_wildcard = tree_wildcard.clone();
                                consume_wildcard(&mut new_tree_wildcard);

                                let mut new_path_wildcard = path_wildcard.clone();
                                consume_wildcard(&mut new_path_wildcard);
                                let job = Job{
                                    path: &path[..],
                                    path_wildcard_override: Some(new_path_wildcard),
                                    tree: child,
                                    tree_wildcard_override: Some(new_tree_wildcard)
                                    };
                                jobs.push(job);
                            }
                        }
                    }

                }
            }
        }

        return result;

    }
}

#[test]
#[ignore = "tofireasonx"]
fn test_add_payload_to_root()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&[Root], "data");
    assert!(tree.element == Root);
    assert!(tree.childs.len() == 0);
    assert!(tree.payloads.len() == 1);
    assert!(tree.payloads[0] == "data");
}

#[test]
fn test_add_payload_to_2root()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&[Root, Root], "data");
    assert!(tree.element == Root);
    assert!(tree.childs.len() == 0);
    assert!(tree.payloads.len() == 1);
    assert!(tree.payloads[0] == "data");
}

#[test]
#[should_panic]
fn test_add_payload_to_root_in_the_middle()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&[Root, Name("test".into()), Root], "data");
}

#[test]
fn test_add_payload()
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
fn test_add_2payload_same_path()
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
fn test_add_2payload_different_path()
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
fn test_add_2payload_different_deep_path()
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


#[test]
fn test_get_payloads()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    let path = [Root, Name("test".into())];
    tree.add_payload(&path, "data");
    assert!(tree.get_payloads(&[Wildcard((1,0)), Wildcard((1,0))]).len() == 1);
    assert!(tree.get_payloads(&[Wildcard((1,0)), Wildcard((1,0))])[0] == &"data");
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).len() == 1);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))])[0] == &"data");
    assert!(tree.get_payloads(&path).len() == 1);
    assert!(tree.get_payloads(&path)[0] == &"data");
}

#[test]
fn test_get_payloads_relative_path()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    let path = [Root, Name("l1".into()), Name("l2".into())];
    tree.add_payload(&path, "data");
    assert!(tree.childs.len() == 1);
    assert!(tree.childs[0].get_payloads(&[Wildcard((1,0)), Wildcard((1,0))]).len() == 1);
    assert!(tree.childs[0].get_payloads(&[Wildcard((1,0)), Wildcard((1,0))])[0] == &"data");
    assert!(tree.childs[0].childs.len() == 1);
    assert!(tree.childs[0].childs[0].get_payloads(&[Wildcard((1,0))]).len() == 1);
    assert!(tree.childs[0].childs[0].get_payloads(&[Wildcard((1,0))])[0] == &"data");
    assert!(tree.childs[0].get_payloads(&path).len() == 0);
}

#[test]
fn test_get_2payloads_same_path()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    let path = [Root, Name("test".into())];
    tree.add_payload(&path, "data");
    tree.add_payload(&path, "data2");
    assert!(tree.get_payloads(&[Wildcard((1,0)), Wildcard((1,0))]).len() == 2);
    assert!(tree.get_payloads(&[Wildcard((1,0)), Wildcard((1,0))])[0] == &"data");
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).len() == 2);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))])[1] == &"data2");
    assert!(tree.get_payloads(&path).len() == 2);
    assert!(tree.get_payloads(&path)[0] == &"data");
    assert!(tree.get_payloads(&path)[1] == &"data2");
}

#[test]
fn test_get_2payload_different_path()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    let path1 = [Root, Name("test".into())];
    let path2 = [Root, Name("test2".into())];
    tree.add_payload(&path1, "data");
    tree.add_payload(&path2, "data2");
    assert!(tree.get_payloads(&[Root]).len() == 0);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).len() == 2);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))])[0] == &"data");
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))])[1] == &"data2");
    assert!(tree.get_payloads(&path1).len() == 1);
    assert!(tree.get_payloads(&path2).len() == 1);
    assert!(tree.get_payloads(&path1)[0] == &"data");
    assert!(tree.get_payloads(&path2)[0] == &"data2");
}

#[test]
fn test_get_2path_different_deep_path()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    let path1 = [Root, Name("l1".into()), Name("l12".into())];
    let path2 = [Root, Name("l1".into()), Name("l22".into())];
    tree.add_payload(&path1, "data1");
    tree.add_payload(&path2, "data2");
    assert!(tree.get_payloads(&[Root]).len() == 0);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).len() == 0);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0)), Name("l12".into())]).len() == 1);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0)), Name("l12".into())])[0] == &"data1");
    assert!(tree.get_payloads(&[Root, Wildcard((1,0)), Wildcard((1,0))]).len() == 2);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0)), Wildcard((1,0))])[0] == &"data1");
    assert!(tree.get_payloads(&[Root, Wildcard((1,0)), Wildcard((1,0))])[1] == &"data2");

    assert!(tree.get_payloads(&path1).len() == 1);
    assert!(tree.get_payloads(&path2).len() == 1);
    assert!(tree.get_payloads(&path1)[0] == &"data1");
    assert!(tree.get_payloads(&path2)[0] == &"data2");
}