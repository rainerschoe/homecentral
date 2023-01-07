use std::{collections::HashSet, str::FromStr};

use by_address::ByAddress;
use std::fmt;

/// A single component of a Path.
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

impl fmt::Display for PathElement
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self
        {
            Root => write!(f, ""),
            Name(name) => write!(f, "{}", name),
            Wildcard(wc) =>
            match wc
            {
                (1,0) => write!(f, "*"),
                (0,10) => write!(f, "**"),
                (m,o) => write!(f, "*{},{}", m, o)
            }
        }
    }
}

/// A Path represents a selector on data to pubish and subscribe.
/// It can be constructed from a &str.
pub struct Path 
{
    elements: Vec<PathElement>
}

impl fmt::Display for Path
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for element in self.elements.iter().skip_while(|e| matches!(e,Root))
        {
            write!(f, "/{}", element)?
        }
        return fmt::Result::Ok(());
    }
}

impl From<&[PathElement]> for Path {
    fn from(path_slice: &[PathElement]) -> Self {
        Path{elements: path_slice.into()}
    }
}

/// Constructs a Path from a str.
/// Some examples for valid path string encodings:
/// "/first_floor/kitchen/ceiling/lamps/central"
/// "/first_floor/*/*/lamps/*"
///     '*' is equivalent to '*1,0'
///     it matches exactly one path element
/// "/first_floor/**/lamps/**"
///     '**' is equivalent to '*0,10'
///     it optionally matches up to 10 path elements
/// "/first_floor/*2,0/lamps/*1,0"
/// "/first_floor/*0,10/lamps/*1,10"
///     Generic wildcards are encoded as follows:
///     '*M,O'
///     M is an integer denoting the mandatory number of path
///       elements this wildcard needs to consume
///     O is an integer denoting the optional number of path
///       elements this wildcard may to consume
///     In the current implementation, the maximum value of M and O is 10
/// "/first_floor/kitchen*/ceiling/lamps/central"
///     NOTE: 'kitchen*' is not and does not contain a wildcard.
///           Only path elements starting with '*' are considered wildcards
impl FromStr for Path
{
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err>
    {
        if ! s.starts_with("/")
        {
            return Err("Path needs to start with '/'".into());
        }
        if s.ends_with("/") && s.len() > 1
        {
            return Err("Path may not end with '/'".into());
        }
        let components = s.split("/").skip_while(|x| x.eq(&""));

        let mut result = Path{elements: Vec::new()};
        result.elements.push(Root);
        for element in components
        {
            result.elements.push(element.parse::<PathElement>()?);
        }
        Ok(result)
    }
}

/// Root element cannot be constructed from string, as it has no string representation
impl FromStr for PathElement
{
    type Err = String;
    fn from_str(s: &str) -> Result<Self, Self::Err>
    {
        if s.eq("") 
        {
            Err("Empty path element not allowed".into())
        }
        else if s.starts_with("*")
        {
            match s
            {
                "*" => Ok(Wildcard((1,0))),
                "**" => Ok(Wildcard((0,9))),
                _ => {
                    let (min_str, opt_str) = s
                    .strip_prefix('*')
                    .and_then(|s| s.split_once(','))
                    .ok_or("Failed to parse Wildcard. Possible Wildcard variants: '*' single mandatory match, '**' Multiple optional matches, '*M,O' with M number of minimum required matches and O number of optional matches. e.g. '*1,0'")?;
                    let min = min_str.parse::<usize>().map_err(|_| "Failed to parse Wildcard: Min number of matches not decodable")?;
                    let opt = opt_str.parse::<usize>().map_err(|_| "Failed to parse Wildcard: Optional number of matches not decodable")?;
                    if min > 10
                    {
                        return Err("Wildcards are only allowed to match up to 10 mandatory elements.".into());
                    }
                    if opt > 10
                    {
                        return Err("Wildcards are only allowed to match up to 10 optional elements.".into());
                    }
                    Ok(Wildcard((min,opt)))
                }
            }
        }
        else
        {
            Ok(Name(s.into()))
        }
    }
}

// subtracts 1 from wildcard, preferred from mandatory count, if this is 0 from optional count.
// returns true if wildcard is fully consumed.
fn consume_wildcard(wildcard: &mut (usize, usize)) -> bool
{
    if wildcard.0 > 0
    {
        wildcard.0 -= 1;
    }
    else if wildcard.1 > 0{
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

impl<T> fmt::Display for PathTree<T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.format_internal(f, 0)
    }
}

struct Job<'path, 'tree, T>
{
    path: &'path [PathElement],
    path_wildcard_override: Option<(usize,usize)>,

    tree: &'tree PathTree<T>,
    tree_wildcard_override: Option<(usize,usize)>,

    parent_node: Option<&'tree PathTree<T>>,
}

struct UniqueReferenceList<'payload, T>
{
    // Hack: rust references do not implement hash and eq traits. this is why I use an additional hashset with the ByAddress crate to check for duplicates. I did not want to expose the ByAddress crate in the user facing APU this is why i need to double maintain the result list here.
    hashset : HashSet<ByAddress<&'payload T>>,
    vector : Vec<&'payload T>
}

impl<'payload, T> UniqueReferenceList<'payload, T>
{
    // NOTE: why can't I #[derive(Default)] here?
    // trait bound `T: Default` was not satisfied
    // Do not understand, as I am only using references here?
    fn new() -> Self
    {
        Self{hashset: HashSet::new(), vector: Vec::new()}
    }

    fn append(self: &mut Self, payloads: &'payload Vec<T>)
    {
        for payload in payloads.iter()
        {
            if self.hashset.insert(ByAddress(&payload))
            {
                self.vector.push(payload);
            }
        }
    }
}

impl<T> PathTree<T>
{
    pub fn new() -> Self
    {
        use PathElement::*;
        PathTree{element: Root, payloads: Vec::new(), childs: Vec::new()}
    }

    fn format_internal(&self, f: &mut fmt::Formatter<'_>, indentation_level: usize) -> fmt::Result
    {
        for _ in 0..indentation_level
        {
            write!(f, "  ")?;
        }
        write!(f, "{}/", self.element)?;
        if self.payloads.len() > 0
        {
            write!(f, " ({})", self.payloads.len())?;
        }
        write!(f, "\n")?;
        for child in self.childs.iter()
        {
            child.format_internal(f, indentation_level+1)?;
        }
        return fmt::Result::Ok(());
    }

    pub fn add_payload(self: &mut Self, path: &Path, payload: T)
    {
        self.add_payload_internal(& path.elements, payload)
    }

    fn add_payload_internal(self: &mut Self, path: &[PathElement], payload: T)
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
            return self.add_payload_internal(&path[1..], payload);
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

        return child.add_payload_internal(&path[1..], payload)
    }

    pub fn get_payloads<'tree, 'path>(
        self: &'tree Self,
        path: &'path Path
    ) -> Vec<&'tree T>
    {
        let path = path.elements.as_slice();
        let initial_job = Job{
                                    path: path,
                                    path_wildcard_override: None,
                                    tree: self,
                                    tree_wildcard_override: None,
                                    parent_node: None
                                    };

        let mut jobs = Vec::new();
        jobs.push(initial_job);

        // We need unique list to filter out duplicates, which might happen, as different permutations of wildcards might match the same payload multiple times
        let mut results = UniqueReferenceList::<T>::new();

        loop {
            if jobs.is_empty()
            {
                break;
            }
            let job = jobs.pop().unwrap();
            let tree = job.tree;
            let path = job.path;

            let tree_node = &tree.element;
            let path_node = path.get(0);
            match (tree_node, path_node)
            {
                (Root, Some(Root)) =>
                {
                    if path.len() == 1
                    {
                        // all matched and no more things to do for this path
                        // collect the reward:
                        results.append(& tree.payloads);
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
                (Name(_), None) =>
                {
                    // tree expects name, but path is empty -> no matches, nothing to do
                },
                (Name(tree_node_name), Some(Name(path_node_name))) =>
                {
                    // match -> add all childs to job list
                    if tree_node_name == path_node_name
                    {
                        if path.len() == 1
                        {
                            // all matched and no more things to do for this path
                            // collect the reward:
                            results.append(&tree.payloads);
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
                (Name(_), Some(Wildcard(path_wildcard))) =>
                {
                    Self::_handle_path_only_wildcard(&path_wildcard, &job, &mut jobs, &mut results);
                },
                (Wildcard(tree_wildcard), Some(Name(_))) =>
                {
                    Self::_handle_tree_only_wildcard(&tree_wildcard, &job, &mut jobs, &mut results);
                },
                (Wildcard(tree_wildcard), Some(Wildcard(path_wildcard))) =>
                {
                    Self::_handle_double_wildcard(& path_wildcard, &tree_wildcard, &job, &mut jobs, &mut results);
                }
                (Wildcard(tree_wildcard), None) =>
                {
                    if tree_wildcard.0 == 0
                    {
                        // wildcard is optional and may be skipped -> we have a match! add payload:
                        results.append(&tree.payloads);
                    }
                },
            }
        }
        return results.vector;
    }

    fn _handle_double_wildcard<'path, 'tree>(
        path_wildcard: & (usize, usize),
        tree_wildcard: & (usize, usize),
        job: & Job<'path, 'tree, T>,
        jobs: &mut Vec<Job<'path, 'tree, T>>,
        results: &mut UniqueReferenceList<'tree, T>
    )
    {
        if job.path.len() == 1
        {
            // all matched and no more things to do for this path
            // collect the reward:
            results.append(&job.tree.payloads);

            // NOTE: path could is wildcard, still need to traverse deeper

            // TODO: this might be quite inefficient as we might recurse over big wildcards (imaging two 2^64 wildcards fighting)
            // need to somehow detect use-less wildcard permutations
        }

        // tree node is a wildcard => retrieve and override if required:
        let tree_wildcard = match job.tree_wildcard_override
        {
            Some(wc_override) => wc_override,
            None => tree_wildcard.clone()
        };

        // path node also wildcard => also override if required:
        let path_wildcard = match job.path_wildcard_override
        {
            Some(wc_override) => wc_override,
            None => path_wildcard.clone()
        };

        // we reduce this scenario down to a set of single wildcard scenarios by recursively removing/consuming from one wildcard:

        if tree_wildcard.0 == 0 && tree_wildcard.1 == 0
        {
            // invalid tree wildcard
            // remove it and contine
            for child in job.tree.childs.iter()
            {
                let job = Job{
                    path: &job.path[..], // full path, as WC might not be fully consumed
                    path_wildcard_override: None,
                    tree: child,
                    tree_wildcard_override: None,
                    parent_node: Some(&job.tree)
                    };
                jobs.push(job);
            }
            return;
        }

        if path_wildcard.0 == 0 && path_wildcard.1 == 0
        {
            // invalid path wildcard
            // remove it and contine
            let job = Job{
                path: &job.path[1..], // full path, as WC might not be fully consumed
                path_wildcard_override: None,
                tree: job.tree,
                tree_wildcard_override: None,
                parent_node: job.parent_node
                };
            jobs.push(job);
            return;
        }


        // no minumums required, so we might also skip wildcard here:
        if tree_wildcard.0 == 0
        {
            for child in job.tree.childs.iter()
            {
                let job = Job{
                    path: &job.path[..], // full path, as WC might not be fully consumed
                    path_wildcard_override: job.path_wildcard_override,
                    tree: child,
                    tree_wildcard_override: None,
                    parent_node: Some(&job.tree)
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
            path: &job.path[..],
            path_wildcard_override: Some(new_path_wildcard),
            tree: job.tree,
            tree_wildcard_override: Some(new_tree_wildcard),
            parent_node: Some(&job.tree)
            };
        jobs.push(job);

    }

    fn _handle_tree_only_wildcard<'path, 'tree>(
        tree_wildcard: & (usize, usize),
        job: & Job<'path, 'tree, T>,
        jobs: &mut Vec<Job<'path, 'tree, T>>,
        results: &mut UniqueReferenceList<'tree, T>
    )
    {
        // tree node is a wildcard => retrieve and override if required:
        let tree_wildcard = match job.tree_wildcard_override
        {
            Some(wc_override) => wc_override,
            None => tree_wildcard.clone()
        };
        if job.path.len() == 1 
        {
            // all matched and no more things to do for this path
            // collect the reward:
            results.append(&job.tree.payloads);
            return;
        }
        if tree_wildcard.0 == 0 && tree_wildcard.1 == 0
        {
            // invalid wildcard
            // remove it and contine
            for child in job.tree.childs.iter()
            {
                let job = Job{
                    path: &job.path[..], // full path, as WC might not be fully consumed
                    path_wildcard_override: None,
                    tree: child,
                    tree_wildcard_override: None,
                    parent_node: Some(&job.tree)
                    };
                jobs.push(job);
            }
            return;
        }

        // no minumums required, so we might also skip wildcard here:
        if tree_wildcard.0 == 0
        {
            for child in job.tree.childs.iter()
            {
                let job = Job{
                    path: &job.path[1..],
                    path_wildcard_override: None,
                    tree: child,
                    tree_wildcard_override: None,
                    parent_node: Some(&job.tree)
                    };
                jobs.push(job);
            }
        }

        // consuming is always an option for valid wildcards:
        let mut new_tree_wildcard = tree_wildcard.clone();
        consume_wildcard(&mut new_tree_wildcard);
        let job = Job{
            path: &job.path[1..],
            path_wildcard_override: None,
            tree: &job.tree,
            tree_wildcard_override: Some(new_tree_wildcard),
            parent_node: Some(&job.tree)
            };
        jobs.push(job);
    }

    fn _handle_path_only_wildcard<'path, 'tree>(
        path_wildcard: & (usize, usize),
        job: & Job<'path, 'tree, T>,
        jobs: &mut Vec<Job<'path, 'tree, T>>,
        results: &mut UniqueReferenceList<'tree, T>
    )
    {
        let path_wildcard = match job.path_wildcard_override
        {
            Some(wc_override) => wc_override,
            None => path_wildcard.clone()
        };
        
        if path_wildcard.0 == 0 && path_wildcard.1 == 0
        {
            // invalid wildcard
            // remove it and contine
            let job = Job{
                path: &job.path[1..], // full path, as WC might not be fully consumed
                path_wildcard_override: None,
                tree: &job.tree,
                tree_wildcard_override: None,
                parent_node: job.parent_node
                };
            jobs.push(job);
            return;
        }

        // When minimums is 0, we also have the choice to NOT consume and skip the wildcard
        if path_wildcard.0 == 0
        {
            if let Some(parent) = job.parent_node
            {
                if job.path.len() == 1
                {
                    // wildcard skipped and it was last in path
                    // -> no need to check nodes at this level, parent already is a match, add its payload:
                    results.append(&parent.payloads);
                }
            }
            // remove it
            let job = Job{
                path: &job.path[1..], // full path, as WC might not be fully consumed
                path_wildcard_override: None,
                tree: &job.tree,
                tree_wildcard_override: None,
                parent_node: job.parent_node
                };
            jobs.push(job);
        }

        if job.path.len() == 1 && path_wildcard.0 <= 1
        {
            // all matched and no more things to do for this path
            // collect the reward:
            results.append(&job.tree.payloads);
        }
        // consuming is always an option:
        // add all childs after consuming one from the wildcard
        for child in job.tree.childs.iter()
        {
            let mut new_path_wildcard = path_wildcard.clone();
            consume_wildcard(&mut new_path_wildcard);
            let new_job = Job{
                path: &job.path[..], // full path, as WC might not be fully consumed
                path_wildcard_override: Some(new_path_wildcard),
                tree: child,
                tree_wildcard_override: None,
                parent_node: Some(&job.tree)
                };
            jobs.push(new_job);
        }
    }
}

#[test]
fn test_add_payload_to_root()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&"/".parse().unwrap(), "data");
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
    tree.add_payload(&([Root, Root][..]).into(), "data");
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
    tree.add_payload(&([Root, Name("test".into()), Root][..]).into(), "data");
}

#[test]
#[should_panic]
fn test_add_payload_to_root_in_the_middle_str()
{
    let _path : Path = "/test/".parse().unwrap();
}

#[test]
fn test_add_payload()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&"/test".parse().unwrap(), "data");
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
    tree.add_payload(&"/test".parse().unwrap(), "data");
    tree.add_payload(&"/test".parse().unwrap(), "data2");
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
    tree.add_payload(&"/test".parse().unwrap(), "data");
    tree.add_payload(&"/test2".parse().unwrap(), "data2");
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
    tree.add_payload(&"/l1/l12".parse().unwrap(), "data1");
    tree.add_payload(&"/l2/l22".parse().unwrap(), "data2");
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
    let mut tree = PathTree::<&str>::new();
    tree.add_payload(&"/test".parse().unwrap(), "data");
    assert!(tree.get_payloads(&"/test".parse().unwrap()).len() == 1);
    assert!(tree.get_payloads(&"/test".parse().unwrap()).contains(&&"data"));
    assert!(tree.get_payloads(&"/*".parse().unwrap()).len() == 1);
    assert!(tree.get_payloads(&"/*".parse().unwrap()).contains(&&"data"));
}

#[test]
fn test_get_payloads_relative_path()
{
    use PathElement::*;
    let mut tree = PathTree::<&str>::new();
    let path = "/l1/l2".parse().unwrap();
    tree.add_payload(&path, "data");
    assert!(tree.childs.len() == 1);
    assert!(tree.childs[0].get_payloads(&[Wildcard((1,0)), Wildcard((1,0))][..].into()).len() == 1);
    assert!(tree.childs[0].get_payloads(&[Wildcard((1,0)), Wildcard((1,0))][..].into()).contains(&&"data"));
    assert!(tree.childs[0].childs.len() == 1);
    assert!(tree.childs[0].childs[0].get_payloads(&[Wildcard((1,0))][..].into()).len() == 1);
    assert!(tree.childs[0].childs[0].get_payloads(&[Wildcard((1,0))][..].into()).contains(&&"data"));
    assert!(tree.childs[0].get_payloads(&path).len() == 0); // from child0 started
}

#[test]
fn test_get_2payloads_same_path()
{
    let mut tree = PathTree::<&str>::new();
    let path = "/test".parse().unwrap();
    tree.add_payload(&path, "data");
    tree.add_payload(&path, "data2");
    assert!(tree.get_payloads(&"/*".parse().unwrap()).len() == 2);
    assert!(tree.get_payloads(&"/*".parse().unwrap()).contains(&&"data"));
    assert!(tree.get_payloads(&"/*".parse().unwrap()).len() == 2);
    assert!(tree.get_payloads(&"/*".parse().unwrap()).contains(&&"data2"));
    assert!(tree.get_payloads(&path).len() == 2);
    assert!(tree.get_payloads(&path).contains(&&"data"));
    assert!(tree.get_payloads(&path).contains(&&"data2"));
}

#[test]
fn test_get_2payload_different_path()
{
    let mut tree = PathTree::<&str>::new();
    let path1 = "/test".parse().unwrap();
    let path2 = "/test2".parse().unwrap();
    tree.add_payload(&path1, "data");
    tree.add_payload(&path2, "data2");
    assert!(tree.get_payloads(&"/".parse().unwrap()).len() == 0);
    assert!(tree.get_payloads(&"/*1,0".parse().unwrap()).len() == 2);
    assert!(tree.get_payloads(&"/*1,0".parse().unwrap()).contains(&&"data"));
    assert!(tree.get_payloads(&"/*1,0".parse().unwrap()).contains(&&"data2"));
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
    let path1 = [Root, Name("l1".into()), Name("l12".into())][..].into();
    let path2 = [Root, Name("l1".into()), Name("l22".into())][..].into();
    tree.add_payload(&path1, "data1");
    tree.add_payload(&path2, "data2");
    assert!(tree.get_payloads(&"/".parse().unwrap()).len() == 0);
    assert!(tree.get_payloads(&"/*".parse().unwrap()).len() == 0);
    assert!(tree.get_payloads(&"/*/l12".parse().unwrap()).len() == 1);
    assert!(tree.get_payloads(&"/*/l12".parse().unwrap()).contains(&&"data1"));
    assert!(tree.get_payloads(&"/*/*".parse().unwrap()).len() == 2);
    assert!(tree.get_payloads(&"/*/*".parse().unwrap()).contains(&&"data1"));
    assert!(tree.get_payloads(&"/*/*".parse().unwrap()).contains(&&"data2"));

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
    tree.add_payload(&[Root][..].into(), "sroot");
    tree.add_payload(&[Root, Name("l1".into())][..].into(), "s1");
    tree.add_payload(&[Root, Name("l1".into()), Name("l11".into())][..].into(), "s11");
    tree.add_payload(&[Root, Name("l1".into()), Name("l12".into())][..].into(), "s12");
    tree.add_payload(&[Root, Name("l2".into())][..].into(), "s2");
    tree.add_payload(&[Root, Name("l2".into()), Name("l21".into())][..].into(), "s21");
    tree.add_payload(&[Root, Name("l2".into()), Name("l22".into())][..].into(), "s22");

    assert!(tree.get_payloads(&[Root][..].into()).len() == 1);
    assert!(tree.get_payloads(&[Root][..].into()).contains(&&"sroot"));

    assert!(tree.get_payloads(&[Root, Name("l1".into())][..].into()).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l1".into())][..].into()).contains(&&"s1"));

    assert!(tree.get_payloads(&[Root, Name("l1".into()), Name("l11".into())][..].into()).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l1".into()), Name("l11".into())][..].into()).contains(&&"s11"));

    assert!(tree.get_payloads(&[Root, Name("l1".into()), Name("l12".into())][..].into()).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l1".into()), Name("l12".into())][..].into()).contains(&&"s12"));

    assert!(tree.get_payloads(&[Root, Name("l2".into())][..].into()).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l2".into())][..].into()).contains(&&"s2"));

    assert!(tree.get_payloads(&[Root, Name("l2".into()), Name("l21".into())][..].into()).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l2".into()), Name("l21".into())][..].into()).contains(&&"s21"));

    assert!(tree.get_payloads(&[Root, Name("l2".into()), Name("l22".into())][..].into()).len() == 1);
    assert!(tree.get_payloads(&[Root, Name("l2".into()), Name("l22".into())][..].into()).contains(&&"s22"));

    assert!(tree.get_payloads(&[Root, Name("l2".into()), Wildcard((1,0))][..].into()).len() == 2);
    assert!(tree.get_payloads(&[Root, Name("l2".into()), Wildcard((1,0))][..].into()).contains(&&"s21"));
    assert!(tree.get_payloads(&[Root, Name("l2".into()), Wildcard((1,0))][..].into()).contains(&&"s22"));

    assert!(tree.get_payloads(&[Root, Wildcard((1,0))][..].into()).len() == 2);
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))][..].into()).contains(&&"s1"));
    assert!(tree.get_payloads(&[Root, Wildcard((1,0))][..].into()).contains(&&"s2"));

    let results = tree.get_payloads(&[Root, Wildcard((0,1))][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 3);
    assert!(results.contains(&&"s1"));
    assert!(results.contains(&&"s2"));
    assert!(results.contains(&&"sroot"));

    let results = tree.get_payloads(&[Root, Wildcard((0,2))][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 7);
    assert!(results.contains(&&"s1"));
    assert!(results.contains(&&"s11"));
    assert!(results.contains(&&"s12"));
    assert!(results.contains(&&"s2"));
    assert!(results.contains(&&"s21"));
    assert!(results.contains(&&"s22"));
    assert!(results.contains(&&"sroot"));

    let results = tree.get_payloads(&[Root, Wildcard((1,1))][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 6);
    assert!(results.contains(&&"s1"));
    assert!(results.contains(&&"s11"));
    assert!(results.contains(&&"s12"));
    assert!(results.contains(&&"s2"));
    assert!(results.contains(&&"s21"));
    assert!(results.contains(&&"s22"));

    let results = tree.get_payloads(&[Root, Wildcard((2,0))][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 4);
    assert!(results.contains(&&"s11"));
    assert!(results.contains(&&"s12"));
    assert!(results.contains(&&"s21"));
    assert!(results.contains(&&"s22"));

    let results = tree.get_payloads(&[Root, Wildcard((3,0))][..].into());
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
    tree.add_payload(&[Root][..].into(), "sroot");
    tree.add_payload(&[Root, Name("same".into())][..].into(), "srootsame");
    tree.add_payload(&[Root, Name("l1".into())][..].into(), "s1");
    tree.add_payload(&[Root, Name("l1".into()), Name("same".into())][..].into(), "s1same");
    tree.add_payload(&[Root, Name("l1".into()), Name("l12".into())][..].into(), "s12");
    tree.add_payload(&[Root, Name("l2".into())][..].into(), "s2");
    tree.add_payload(&[Root, Name("l2".into()), Name("l21".into())][..].into(), "s21");
    tree.add_payload(&[Root, Name("l2".into()), Name("l22".into())][..].into(), "s22");
    tree.add_payload(&[Root, Name("l2".into()), Name("same".into())][..].into(), "s2same");

    let results = tree.get_payloads(&[Root, Wildcard((1,0)), Name("same".into())][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 2);
    assert!(results.contains(&&"s1same"));
    assert!(results.contains(&&"s2same"));

    let results = tree.get_payloads(&[Root, Wildcard((0,1)), Name("same".into())][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 3);
    assert!(results.contains(&&"s1same"));
    assert!(results.contains(&&"s2same"));
    assert!(results.contains(&&"srootsame"));

    let results = tree.get_payloads(&[Root, Wildcard((1,1)), Name("same".into())][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 2);
    assert!(results.contains(&&"s1same"));
    assert!(results.contains(&&"s2same"));

    let results = tree.get_payloads(&[Root, Wildcard((0,10)), Name("same".into())][..].into());
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
    tree.add_payload(&[Root][..].into(), "sroot");
    tree.add_payload(&[Root, Name("same".into())][..].into(), "srootsame");
    tree.add_payload(&[Root, Wildcard((0,100))][..].into(), "severything");
    tree.add_payload(&[Root, Wildcard((0,100)), Name("light".into())][..].into(), "sanyLight");
    tree.add_payload(&[Root, Name("l1".into())][..].into(), "s1");
    tree.add_payload(&[Root, Name("l1".into()), Name("l11".into())][..].into(), "s11");
    tree.add_payload(&[Root, Name("l2".into())][..].into(), "s2");
    tree.add_payload(&[Root, Name("l2".into()), Name("l21".into())][..].into(), "s21");
    tree.add_payload(&[Root, Name("l2".into()), Name("light".into())][..].into(), "s2light");
    tree.add_payload(&[Root, Name("l2".into()), Name("l22".into())][..].into(), "s22");
    tree.add_payload(&[Root, Name("l2".into()), Name("l22".into())][..].into(), "s22");
    tree.add_payload(&[Root, Name("l2".into()), Wildcard((1,0))][..].into(), "s2x");
    tree.add_payload(&[Root, Name("l2".into()), Wildcard((0,1))][..].into(), "s2opt");

    let results = tree.get_payloads(&[Root, Name("l1".into())][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 2);
    assert!(results.contains(&&"s1"));
    assert!(results.contains(&&"severything"));

    let results = tree.get_payloads(&[Root, Name("l2".into()), Name("light".into())][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 5);
    assert!(results.contains(&&"s2opt"));
    assert!(results.contains(&&"s2x"));
    assert!(results.contains(&&"s2light"));
    assert!(results.contains(&&"sanyLight"));
    assert!(results.contains(&&"severything"));

    let results = tree.get_payloads(&[Root, Name("l1".into()), Name("light".into())][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 2);
    assert!(results.contains(&&"sanyLight"));
    assert!(results.contains(&&"severything"));

    let results = tree.get_payloads(&[Root, Name("l2".into())][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 3);
    assert!(results.contains(&&"s2"));
    assert!(results.contains(&&"s2opt"));
    assert!(results.contains(&&"severything"));
}

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
    tree.add_payload(&[Root][..].into() , "sroot");
    tree.add_payload(&[Root, Name("same".into())][..].into(), "srootsame");
    tree.add_payload(&[Root, Wildcard((0,100))][..].into(), "severything");
    tree.add_payload(&[Root, Wildcard((0,100)), Name("light".into())][..].into(), "sanyLight");
    tree.add_payload(&[Root, Name("l1".into())][..].into(), "s1");
    tree.add_payload(&[Root, Name("l1".into()), Name("l11".into())][..].into(), "s11");
    tree.add_payload(&[Root, Name("l2".into())][..].into(), "s2");
    tree.add_payload(&[Root, Name("l2".into()), Name("l21".into())][..].into(), "s21");
    tree.add_payload(&[Root, Name("l2".into()), Name("light".into())][..].into(), "s2light");
    tree.add_payload(&[Root, Name("l2".into()), Name("l22".into())][..].into(), "s22");
    tree.add_payload(&[Root, Name("l2".into()), Wildcard((1,0))][..].into(), "s2x");
    tree.add_payload(&[Root, Name("l2".into()), Wildcard((0,1))][..].into(), "s2opt");

    println!("Tree:\n{}", tree);
    let results = tree.get_payloads(&[Root, Wildcard((0,10))][..].into());
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

    let results = tree.get_payloads(&[Root, Wildcard((0,10)), Name("light".into())][..].into());
    println!("res={:#?}", results);
    assert!(results.len() == 3);
    assert!(results.contains(&&"severything"));
    assert!(results.contains(&&"sanyLight"));
    assert!(results.contains(&&"s2light"));
}