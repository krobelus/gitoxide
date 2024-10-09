use gix_diff::Rewrites;

/// The error returned by [`tree()`](crate::tree()).
#[derive(Debug, thiserror::Error)]
#[allow(missing_docs)]
pub enum Error {
    #[error("Could not find ancestor, our or their tree to get started")]
    FindTree(#[from] gix_object::find::existing_object::Error),
    #[error("Could not find ancestor, our or their tree to get started")]
    FindTree2(#[from] gix_object::find::existing_iter::Error),
    #[error("Failed to diff our side or their side")]
    DiffTree(#[from] gix_diff::tree_with_rewrites::Error),
}

/// The outcome produced by [`tree()`](crate::tree()).
pub struct Outcome<'a> {
    /// The ready-made (but unwritten) tree if `conflicts` is empty, or the best-possible tree when facing `conflicts`.
    ///
    /// The tree may contain blobs with conflict markers, and will be missing directories or files that were conflicting
    /// without a resolution strategy.
    tree: gix_object::tree::Editor<'a>,
    /// The set of conflicts we encountered. Can be empty to indicate there was no conflict.
    conflicts: Vec<Conflict>,
}

/// A description of a conflict (i.e. merge issue without an auto-resolution) as seen during a [tree-merge](crate::tree()).
pub struct Conflict;

/// A way to configure [`tree()`](crate::tree()).
#[derive(Default, Debug, Copy, Clone)]
pub struct Options {
    /// If not `None`, rename tracking will be performed when determining the changes of each side of the merge.
    pub rewrites: Option<Rewrites>,
    // TODO(Perf) add a flag to allow parallelizing the tree-diff itself.
}

pub(super) mod function {
    use crate::tree::{Error, Options, Outcome};
    use gix_diff::tree::recorder::Location;
    use gix_object::{FindExt, TreeRefIter};
    use std::convert::Infallible;

    /// Perform a merge between `our_tree` and `their_tree`, using `base_tree` as merge-base.
    /// Note that `base_tree` can be an empty tree to indicate 'no common ancestor between the two sides'.
    ///
    /// `labels` are relevant for text-merges and will be shown in conflicts.
    /// `objects` provides access to trees when diffing them.
    /// `diff_state` is state used for diffing trees.
    /// `diff_resource_cache` is used for similarity checks.
    /// `blob_merge` is a pre-configured platform to merge any content.
    /// `options` are used to affect how the merge is performed.
    ///
    /// ### Performance
    ///
    /// Note that `objects` *should* have an object cache to greatly accelerate tree-retrieval.
    pub fn tree<'a>(
        base_tree: &gix_hash::oid,
        our_tree: &gix_hash::oid,
        their_tree: &gix_hash::oid,
        labels: crate::blob::builtin_driver::text::Labels<'_>,
        objects: &impl gix_object::FindObjectOrHeader,
        diff_state: &mut gix_diff::tree::State,
        diff_resource_cache: &mut gix_diff::blob::Platform,
        blob_merge: &mut crate::blob::Platform,
        options: Options,
    ) -> Result<Outcome<'a>, Error> {
        let (mut base_buf, mut side_buf) = (Vec::new(), Vec::new());
        let ancestor_tree = objects.find_tree_iter(base_tree, &mut base_buf)?;
        let our_tree = objects.find_tree_iter(our_tree, &mut side_buf)?;

        let mut our_changes = Vec::new();
        gix_diff::tree_with_rewrites(
            ancestor_tree,
            our_tree,
            diff_resource_cache,
            diff_state,
            objects,
            |change| -> Result<_, Infallible> {
                our_changes.push(change.into_owned());
                Ok(gix_diff::tree_with_rewrites::Action::Continue)
            },
            gix_diff::tree_with_rewrites::Options {
                location: Some(Location::Path),
                rewrites: options.rewrites,
            },
        )?;

        let mut their_changes = Vec::new();
        let their_tree = objects.find_tree_iter(their_tree, &mut side_buf)?;
        gix_diff::tree_with_rewrites(
            ancestor_tree,
            their_tree,
            diff_resource_cache,
            diff_state,
            objects,
            |change| -> Result<_, Infallible> {
                their_changes.push(change.into_owned());
                Ok(gix_diff::tree_with_rewrites::Action::Continue)
            },
            gix_diff::tree_with_rewrites::Options {
                location: Some(Location::Path),
                rewrites: options.rewrites,
            },
        )?;

        dbg!(&our_changes, &their_changes);
        let mut editor = gix_object::tree::Editor::new(
            gix_object::TreeRef::from_bytes(&base_buf)
                .expect("ancestor was decoded before")
                .into(),
            &objects,
            base_tree.kind(),
        );
        todo!()
    }
}
