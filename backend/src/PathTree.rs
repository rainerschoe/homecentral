use std::collections::HashSet;

use by_address::ByAddress;

#[derive(PartialEq, Clone, Debug)]
pub enum PathElement
{
    // we need a root element, which is only allowed at beginning of path. reason is, that we might have subscribers like this: s1:"/a" s2:"/b", s3:"/*/g", each starting with different path elements. however in a tree structure, we must start from a single node, which we define as Root here.
    // Alternatively, user would need to provide a root node when creating the Tree. however this requires always specifying the same node on each subscribe and publish by the user.
    Root,

    // A string element
    Name(String),

    // (Min, Max) : Number of consumed nodes are any between Min and Max inclusive.
    Wildcard((usize, usize))
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

pub struct PathTree<T>
{
    element: PathElement,
    payloads: Vec<T>,
    childs: Vec<PathTree<T>>,
}

impl<T> PathTree<T>
{
    pub fn new() -> Self
    {
        use PathElement::*;
        PathTree{element: Root, payloads: Vec::new(), childs: Vec::new()}
    }

    fn collect_results<'a>(hashset : &mut HashSet<ByAddress<&'a T>>, results : &mut Vec<&'a T>, payloads: &'a Vec<T>)
    {
        for payload in payloads.iter()
        {
            if hashset.insert(ByAddress(&payload))
            {
                results.push(payload);
            }
        }
    }

    pub fn add_payload(self: &mut Self, path: &[PathElement], payload: T)
    {
        use PathElement::*;
        if path.is_empty()
        {
            // add payload at current level
            self.payloads.push(payload);
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

    pub fn get_payloads<'a, 'b>(self: &'a Self, path: &'b [PathElement]) -> Vec<&'a T>
    {
        //println!("-----");
        struct Job<'c, 'd, T>
        {
            path: &'c [PathElement],
            path_wildcard_override: Option<(usize,usize)>,

            tree: &'d PathTree<T>,
            tree_wildcard_override: Option<(usize,usize)>,

            parent_node: Option<&'d PathTree<T>>,
        }

        let initial_job = Job{
                                    path: path,
                                    path_wildcard_override: None,
                                    tree: self,
                                    tree_wildcard_override: None,
                                    parent_node: None
                                    };

        let mut jobs = Vec::new();
        jobs.push(initial_job);

        // Hack: rust references do not implement hash and eq traits. this is why I use an additional hashset with the ByAddress crate to check for duplicates. I did not want to expose the ByAddress crate in the user facing APU this is why i need to double maintain the result list here:
        //let mut result_hashmap : HashSet<usize> = HashSet::new();
        // We need all this to filter out duplicates, which might happen, as different permutations of wildcards might match the same payload multiple times
        let mut result_hashmap : HashSet<ByAddress<&T>> = HashSet::new();
        let mut result : Vec<&'a T>= Vec::new();
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

            println!("job: tree={:?}(override: {:?}) path={:?}(override: {:?})", tree.element, tree_wildcard_override, path, path_wildcard_override);

            let tree_node = &tree.element;
            //let path_node = &path[0];
            let path_node = path.get(0);
            match (tree_node, path_node)
            {
                (Root, Some(Root)) =>
                {
                    if path.len() == 1
                    {
                        // all matched and no more things to do for this path
                        // collect the reward:
                        Self::collect_results(&mut result_hashmap, &mut result, & tree.payloads);
                    }
                    for child in tree.childs.iter()
                    {
                        let job = Job{
                            path: &path[1..],
                            path_wildcard_override: None,
                            tree: child,
                            tree_wildcard_override: None,
                            parent_node: Some(&tree)
                            };
                        jobs.push(job);
                    }
                },
                (Root, _) =>
                {
                    // root needs to match with root, otherwise path is malformed and will lead to no results at all.
                    return Vec::new();
                },
                (_, Some(Root)) =>
                {
                    return Vec::new();
                },
                (Name(tree_node_name), None) =>
                {
                    // tree expects name, but path is empty -> no matches, nothing to do
                },
                (Name(tree_node_name), Some(Name(path_node_name))) =>
                {
                    // match -> add all childs to job list
                    if tree_node_name == path_node_name
                    {
                        //if path.len() == 1 && tree.childs.len() == 0
                        if path.len() == 1
                        {
                            // all matched and no more things to do for this path
                            // collect the reward:
                            Self::collect_results(&mut result_hashmap, &mut result, & tree.payloads);
                        }
                        for child in tree.childs.iter()
                        {
                            let job = Job{
                                path: &path[1..],
                                path_wildcard_override: None,
                                tree: child,
                                tree_wildcard_override: None,
                                parent_node: Some(&tree)
                                };
                            jobs.push(job);
                        }
                    }
                },
                (Name(tree_node_name), Some(Wildcard(path_wildcard))) =>
                {
                    let mut path_wildcard = match job.path_wildcard_override
                    {
                        Some(wc_override) => wc_override,
                        None => path_wildcard.clone()
                    };
                    
                    if path_wildcard.0 == 0 && path_wildcard.1 == 0
                    {
                        // invalid wildcard
                        // remove it and contine
                        let job = Job{
                            path: &path[1..], // full path, as WC might not be fully consumed
                            path_wildcard_override: None,
                            tree: &tree,
                            tree_wildcard_override: None,
                            parent_node: job.parent_node
                            };
                        jobs.push(job);
                        continue 'jobloop;
                    }

                    // When minimums is 0, we also have the choice to NOT consume and skip the wildcard
                    if path_wildcard.0 == 0
                    {
                        if let Some(parent) = job.parent_node
                        {
                            if path.len() == 1
                            {
                                println!("b");
                                // wildcard skipped and it was last in path
                                // -> no need to check nodes at this level, parent already is a match, add its payload:
                                Self::collect_results(&mut result_hashmap, &mut result, & parent.payloads);
                                println!("b!");
                            }
                        }
                        // remove it
                        let job = Job{
                            path: &path[1..], // full path, as WC might not be fully consumed
                            path_wildcard_override: None,
                            tree: &tree,
                            tree_wildcard_override: None,
                            parent_node: job.parent_node
                            };
                        jobs.push(job);

                    }

                    if path.len() == 1 && path_wildcard.0 <= 1
                    {
                        println!("a");
                        // all matched and no more things to do for this path
                        // collect the reward:
                        Self::collect_results(&mut result_hashmap, &mut result, & tree.payloads);
                        println!("a!");
                    }
                    // consuming is always an option:
                    // add all childs after consuming one from the wildcard
                    for child in tree.childs.iter()
                    {
                        let mut new_path_wildcard = path_wildcard.clone();
                        consume_wildcard(&mut new_path_wildcard);
                        let job = Job{
                            path: &path[..], // full path, as WC might not be fully consumed
                            path_wildcard_override: Some(new_path_wildcard),
                            tree: child,
                            tree_wildcard_override: None,
                            parent_node: Some(&tree)
                            };
                        jobs.push(job);
                    }
                },
                (Wildcard(tree_wildcard), None) =>
                {
                    if(tree_wildcard.0 == 0)
                    {
                        // wildcard is optional and may be skipped
                        // -> we have a match! add payload:
                        Self::collect_results(&mut result_hashmap, &mut result, & tree.payloads);
                    }
                },
                (Wildcard(tree_wildcard), Some(Name(path_node_name))) =>
                {
                    // tree node is a wildcard => retrieve and override if required:
                    let mut tree_wildcard = match job.tree_wildcard_override
                    {
                        Some(wc_override) => wc_override,
                        None => tree_wildcard.clone()
                    };
                    if path.len() == 1 
                    {
                        // all matched and no more things to do for this path
                        // collect the reward:
                        Self::collect_results(&mut result_hashmap, &mut result, & tree.payloads);
                        continue 'jobloop;
                    }
                    if tree_wildcard.0 == 0 && tree_wildcard.1 == 0
                    {
                        // invalid wildcard
                        // remove it and contine
                        for child in tree.childs.iter()
                        {
                            let job = Job{
                                path: &path[..], // full path, as WC might not be fully consumed
                                path_wildcard_override: None,
                                tree: child,
                                tree_wildcard_override: None,
                                parent_node: Some(&tree)
                                };
                            jobs.push(job);
                        }
                        continue 'jobloop;
                    }

                    // no minumums required, so we might also skip wildcard here:
                    if tree_wildcard.0 == 0
                    {
                        for child in tree.childs.iter()
                        {
                            let job = Job{
                                path: &path[1..],
                                path_wildcard_override: None,
                                tree: child,
                                tree_wildcard_override: None,
                                parent_node: Some(&tree)
                                };
                            jobs.push(job);
                        }
                    }

                    // consuming is always an option for valid wildcards:
                    let mut new_tree_wildcard = tree_wildcard.clone();
                    consume_wildcard(&mut new_tree_wildcard);
                    let job = Job{
                        path: &path[1..],
                        path_wildcard_override: None,
                        tree: &tree,
                        tree_wildcard_override: Some(new_tree_wildcard),
                        parent_node: Some(&tree)
                        };
                    jobs.push(job);
                },
                (Wildcard(tree_wildcard), Some(Wildcard(path_wildcard))) =>
                {
                    if path.len() == 1
                    {
                        // all matched and no more things to do for this path
                        // collect the reward:
                        Self::collect_results(&mut result_hashmap, &mut result, & tree.payloads);

                        // NOTE: path could is wildcard, still need to traverse deeper

                        // TODO: this might be quite inefficient as we might recurse over big wildcards (imaging two 2^64 wildcards fighting)
                        // need to somehow detect use-less wildcard permutations
                    }
                    // tree node is a wildcard => retrieve and override if required:
                    let mut tree_wildcard = match job.tree_wildcard_override
                    {
                        Some(wc_override) => wc_override,
                        None => tree_wildcard.clone()
                    };
                    // path node also wildcard => also override if required:
                    let mut path_wildcard = match job.path_wildcard_override
                    {
                        Some(wc_override) => wc_override,
                        None => path_wildcard.clone()
                    };

                    // we reduce this scenario down to a set of single wildcard scenarios by recursively removing/consuming from one wildcard:

                    if tree_wildcard.0 == 0 && tree_wildcard.1 == 0
                    {
                        // invalid tree wildcard
                        // remove it and contine
                        for child in tree.childs.iter()
                        {
                            let job = Job{
                                path: &path[..], // full path, as WC might not be fully consumed
                                path_wildcard_override: None,
                                tree: child,
                                tree_wildcard_override: None,
                                parent_node: Some(&tree)
                                };
                            jobs.push(job);
                        }
                        continue 'jobloop;
                    }

                    if path_wildcard.0 == 0 && path_wildcard.1 == 0
                    {
                        // invalid path wildcard
                        // remove it and contine
                        let job = Job{
                            path: &path[1..], // full path, as WC might not be fully consumed
                            path_wildcard_override: None,
                            tree: tree,
                            tree_wildcard_override: None,
                            parent_node: job.parent_node
                            };
                        jobs.push(job);
                        continue 'jobloop;
                    }


                    // no minumums required, so we might also skip wildcard here:
                    if tree_wildcard.0 == 0
                    {
                        for child in tree.childs.iter()
                        {
                            let job = Job{
                                path: &path[..], // full path, as WC might not be fully consumed
                                path_wildcard_override: job.path_wildcard_override,
                                tree: child,
                                tree_wildcard_override: None,
                                parent_node: Some(&tree)
                                };
                            jobs.push(job);
                        }
                    }

                    // consuming is always an option for valid wildcards:
                    let mut new_tree_wildcard = tree_wildcard.clone();
                    consume_wildcard(&mut new_tree_wildcard);

                    let mut new_path_wildcard = path_wildcard.clone();
                    consume_wildcard(&mut new_path_wildcard);
                    let job = Job{
                        path: &path[..],
                        path_wildcard_override: Some(new_path_wildcard),
                        tree: tree,
                        tree_wildcard_override: Some(new_tree_wildcard),
                        parent_node: Some(&tree)
                        };
                    jobs.push(job);
                }
            }
        }

        return result;
    }
}

#[test]
//#[ignore = "tofireasonx"]
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
    assert!(tree.get_payloads(&[Root, Name("test".into())]).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("test".into())]).contains(&&"data"));
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).len() == 1);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).contains(&&"data"));
    //assert!(tree.get_payloads(&path).len() == 1);
    //assert!(tree.get_payloads(&path)[0] == &"data");
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
    assert!(tree.childs[0].get_payloads(&[Wildcard((1,0)), Wildcard((1,0))]).contains(&&"data"));
    assert!(tree.childs[0].childs.len() == 1);
    assert!(tree.childs[0].childs[0].get_payloads(&[Wildcard((1,0))]).len() == 1);
    assert!(tree.childs[0].childs[0].get_payloads(&[Wildcard((1,0))]).contains(&&"data"));
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
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).len() == 2);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).contains(&&"data"));
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).len() == 2);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).contains(&&"data2"));
    assert!(tree.get_payloads(&path).len() == 2);
    assert!(tree.get_payloads(&path).contains(&&"data"));
    assert!(tree.get_payloads(&path).contains(&&"data2"));
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
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).contains(&&"data"));
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).contains(&&"data2"));
    assert!(tree.get_payloads(&path1).len() == 1);
    assert!(tree.get_payloads(&path2).len() == 1);
    assert!(tree.get_payloads(&path1).contains(&&"data"));
    assert!(tree.get_payloads(&path2).contains(&&"data2"));
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
    assert!(tree.get_payloads(&[Root, Wildcard((1,0)), Name("l12".into())]).contains(&&"data1"));
    assert!(tree.get_payloads(&[Root, Wildcard((1,0)), Wildcard((1,0))]).len() == 2);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0)), Wildcard((1,0))]).contains(&&"data1"));
    assert!(tree.get_payloads(&[Root, Wildcard((1,0)), Wildcard((1,0))]).contains(&&"data2"));

    assert!(tree.get_payloads(&path1).len() == 1);
    assert!(tree.get_payloads(&path2).len() == 1);
    assert!(tree.get_payloads(&path1).contains(&&"data1"));
    assert!(tree.get_payloads(&path2).contains(&&"data2"));
}

#[test]
fn test_wildcard_at_end_of_path()
{
    // roota        sroot
    //    l1        s1
    //      l11     s11
    //      l12     s12
    //    l2        s2
    //      l21     s21
    //      l22     s22
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&[Root], "sroot");
    tree.add_payload(&[Root, Name("l1".into())], "s1");
    tree.add_payload(&[Root, Name("l1".into()), Name("l11".into())], "s11");
    tree.add_payload(&[Root, Name("l1".into()), Name("l12".into())], "s12");
    tree.add_payload(&[Root, Name("l2".into())], "s2");
    tree.add_payload(&[Root, Name("l2".into()), Name("l21".into())], "s21");
    tree.add_payload(&[Root, Name("l2".into()), Name("l22".into())], "s22");

    assert!(tree.get_payloads(&[Root]).len() == 1);
    assert!(tree.get_payloads(&[Root]).contains(&&"sroot"));

    assert!(tree.get_payloads(&[Root, Name("l1".into())]).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l1".into())]).contains(&&"s1"));

    assert!(tree.get_payloads(&[Root, Name("l1".into()), Name("l11".into())]).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l1".into()), Name("l11".into())]).contains(&&"s11"));

    assert!(tree.get_payloads(&[Root, Name("l1".into()), Name("l12".into())]).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l1".into()), Name("l12".into())]).contains(&&"s12"));

    assert!(tree.get_payloads(&[Root, Name("l2".into())]).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l2".into())]).contains(&&"s2"));

    assert!(tree.get_payloads(&[Root, Name("l2".into()), Name("l21".into())]).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l2".into()), Name("l21".into())]).contains(&&"s21"));

    assert!(tree.get_payloads(&[Root, Name("l2".into()), Name("l22".into())]).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l2".into()), Name("l22".into())]).contains(&&"s22"));

    assert!(tree.get_payloads(&[Root, Name("l2".into()), Wildcard((1,0))]).len() == 2);
    assert!(tree.get_payloads(&[Root, Name("l2".into()), Wildcard((1,0))]).contains(&&"s21"));
    assert!(tree.get_payloads(&[Root, Name("l2".into()), Wildcard((1,0))]).contains(&&"s22"));

    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).len() == 2);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).contains(&&"s1"));
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))]).contains(&&"s2"));

    let results = tree.get_payloads(&[Root, Wildcard((0,1))]);
    println!("res={:#?}", results);
    assert!(results.len() == 3);
    assert!(results.contains(&&"s1"));
    assert!(results.contains(&&"s2"));
    assert!(results.contains(&&"sroot"));

    let results = tree.get_payloads(&[Root, Wildcard((0,2))]);
    println!("res={:#?}", results);
    assert!(results.len() == 7);
    assert!(results.contains(&&"s1"));
    assert!(results.contains(&&"s11"));
    assert!(results.contains(&&"s12"));
    assert!(results.contains(&&"s2"));
    assert!(results.contains(&&"s21"));
    assert!(results.contains(&&"s22"));
    assert!(results.contains(&&"sroot"));

    let results = tree.get_payloads(&[Root, Wildcard((1,1))]);
    println!("res={:#?}", results);
    assert!(results.len() == 6);
    assert!(results.contains(&&"s1"));
    assert!(results.contains(&&"s11"));
    assert!(results.contains(&&"s12"));
    assert!(results.contains(&&"s2"));
    assert!(results.contains(&&"s21"));
    assert!(results.contains(&&"s22"));

    let results = tree.get_payloads(&[Root, Wildcard((2,0))]);
    println!("res={:#?}", results);
    assert!(results.len() == 4);
    assert!(results.contains(&&"s11"));
    assert!(results.contains(&&"s12"));
    assert!(results.contains(&&"s21"));
    assert!(results.contains(&&"s22"));

    let results = tree.get_payloads(&[Root, Wildcard((3,0))]);
    println!("res={:#?}", results);
    assert!(results.len() == 0);
}

#[test]
fn test_wildcard_in_middle()
{
    // roota        sroot
    //    l1        s1
    //      same    s1same
    //      l12     s12
    //    l2        s2
    //      l21     s21
    //      l22     s22
    //      same    s2same
    //    same      srootsame
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&[Root], "sroot");
    tree.add_payload(&[Root, Name("same".into())], "srootsame");
    tree.add_payload(&[Root, Name("l1".into())], "s1");
    tree.add_payload(&[Root, Name("l1".into()), Name("same".into())], "s1same");
    tree.add_payload(&[Root, Name("l1".into()), Name("l12".into())], "s12");
    tree.add_payload(&[Root, Name("l2".into())], "s2");
    tree.add_payload(&[Root, Name("l2".into()), Name("l21".into())], "s21");
    tree.add_payload(&[Root, Name("l2".into()), Name("l22".into())], "s22");
    tree.add_payload(&[Root, Name("l2".into()), Name("same".into())], "s2same");

    let results = tree.get_payloads(&[Root, Wildcard((1,0)), Name("same".into())]);
    println!("res={:#?}", results);
    assert!(results.len() == 2);
    assert!(results.contains(&&"s1same"));
    assert!(results.contains(&&"s2same"));

    let results = tree.get_payloads(&[Root, Wildcard((0,1)), Name("same".into())]);
    println!("res={:#?}", results);
    assert!(results.len() == 3);
    assert!(results.contains(&&"s1same"));
    assert!(results.contains(&&"s2same"));
    assert!(results.contains(&&"srootsame"));

    let results = tree.get_payloads(&[Root, Wildcard((1,1)), Name("same".into())]);
    println!("res={:#?}", results);
    assert!(results.len() == 2);
    assert!(results.contains(&&"s1same"));
    assert!(results.contains(&&"s2same"));

    let results = tree.get_payloads(&[Root, Wildcard((0,10)), Name("same".into())]);
    println!("res={:#?}", results);
    assert!(results.len() == 3);
    assert!(results.contains(&&"s1same"));
    assert!(results.contains(&&"s2same"));
    assert!(results.contains(&&"srootsame"));
}

#[test]
fn test_wildcard_in_tree()
{
    // roota        sroot
    //    l1        s1
    //      l11     s11
    //    l2        s2
    //      l21     s21
    //      light   s2light
    //      l22     s22
    //      *1,0    s2x
    //      *0,1    s2opt
    //    *0,100    severyting
    //      light   sanyLight
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&[Root], "sroot");
    tree.add_payload(&[Root, Name("same".into())], "srootsame");
    tree.add_payload(&[Root, Wildcard((0,100))], "severything");
    tree.add_payload(&[Root, Wildcard((0,100)), Name("light".into())], "sanyLight");
    tree.add_payload(&[Root, Name("l1".into())], "s1");
    tree.add_payload(&[Root, Name("l1".into()), Name("l11".into())], "s11");
    tree.add_payload(&[Root, Name("l2".into())], "s2");
    tree.add_payload(&[Root, Name("l2".into()), Name("l21".into())], "s21");
    tree.add_payload(&[Root, Name("l2".into()), Name("light".into())], "s2light");
    tree.add_payload(&[Root, Name("l2".into()), Name("l22".into())], "s22");
    tree.add_payload(&[Root, Name("l2".into()), Name("l22".into())], "s22");
    tree.add_payload(&[Root, Name("l2".into()), Wildcard((1,0))], "s2x");
    tree.add_payload(&[Root, Name("l2".into()), Wildcard((0,1))], "s2opt");

    let results = tree.get_payloads(&[Root, Name("l1".into())]);
    println!("res={:#?}", results);
    assert!(results.len() == 2);
    assert!(results.contains(&&"s1"));
    assert!(results.contains(&&"severything"));

    let results = tree.get_payloads(&[Root, Name("l2".into()), Name("light".into())]);
    println!("res={:#?}", results);
    assert!(results.len() == 5);
    assert!(results.contains(&&"s2opt"));
    assert!(results.contains(&&"s2x"));
    assert!(results.contains(&&"s2light"));
    assert!(results.contains(&&"sanyLight"));
    assert!(results.contains(&&"severything"));

    let results = tree.get_payloads(&[Root, Name("l1".into()), Name("light".into())]);
    println!("res={:#?}", results);
    assert!(results.len() == 2);
    assert!(results.contains(&&"sanyLight"));
    assert!(results.contains(&&"severything"));

    let results = tree.get_payloads(&[Root, Name("l2".into())]);
    println!("res={:#?}", results);
    assert!(results.len() == 3);
    assert!(results.contains(&&"s2"));
    assert!(results.contains(&&"s2opt"));
    assert!(results.contains(&&"severything"));
}

//#[ignore]
#[test]
fn test_wildcard_in_tree_and_path()
{
    // roota        sroot
    //    l1        s1
    //      l11     s11
    //    l2        s2
    //      l21     s21
    //      light   s2light
    //      l22     s22
    //      *1,0    s2x
    //      *0,1    s2opt
    //    *0,100    severyting
    //      light   sanyLight
    //    same      srootsame

    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&[Root], "sroot");
    tree.add_payload(&[Root, Name("same".into())], "srootsame");
    tree.add_payload(&[Root, Wildcard((0,100))], "severything");
    tree.add_payload(&[Root, Wildcard((0,100)), Name("light".into())], "sanyLight");
    tree.add_payload(&[Root, Name("l1".into())], "s1");
    tree.add_payload(&[Root, Name("l1".into()), Name("l11".into())], "s11");
    tree.add_payload(&[Root, Name("l2".into())], "s2");
    tree.add_payload(&[Root, Name("l2".into()), Name("l21".into())], "s21");
    tree.add_payload(&[Root, Name("l2".into()), Name("light".into())], "s2light");
    tree.add_payload(&[Root, Name("l2".into()), Name("l22".into())], "s22");
    tree.add_payload(&[Root, Name("l2".into()), Wildcard((1,0))], "s2x");
    tree.add_payload(&[Root, Name("l2".into()), Wildcard((0,1))], "s2opt");

    let results = tree.get_payloads(&[Root, Wildcard((0,10))]);
    println!("res={:#?}", results);
    assert!(results.len() == 12);
    assert!(results.contains(&&"s1"));
    assert!(results.contains(&&"s11"));
    assert!(results.contains(&&"s2"));
    assert!(results.contains(&&"s21"));
    assert!(results.contains(&&"s2light"));
    assert!(results.contains(&&"s22"));
    assert!(results.contains(&&"s2x"));
    assert!(results.contains(&&"s2opt"));
    assert!(results.contains(&&"sanyLight"));
    assert!(results.contains(&&"severything"));
    assert!(results.contains(&&"sroot"));
    assert!(results.contains(&&"srootsame"));

    let results = tree.get_payloads(&[Root, Wildcard((0,10)), Name("light".into())]);
    println!("res={:#?}", results);
    assert!(results.len() == 3);
    assert!(results.contains(&&"severything"));
    assert!(results.contains(&&"sanyLight"));
    assert!(results.contains(&&"s2light"));
}