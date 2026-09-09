//! Preserve keyed native child ownership until the host is ready to lower.
use iced::{
    Alignment, Element, Length, Padding, Pixels, advanced::Renderer, widget::keyed::Column,
};

pub struct KeyedColumnRecipe<'a, Key, Message, Theme = iced::Theme, R = iced::Renderer>
where
    Key: Copy + PartialEq,
    R: Renderer,
{
    keys: Vec<Key>,
    children: Vec<Element<'a, Message, Theme, R>>,
    width: Length,
    height: Length,
    spacing: Option<f32>,
    padding: Option<Padding>,
    max_width: Option<f32>,
    align_items: Option<Alignment>,
}
impl<'a, K: Copy + PartialEq, M, Theme, R: Renderer> KeyedColumnRecipe<'a, K, M, Theme, R> {
    pub(super) fn from_vecs(keys: Vec<K>, children: Vec<Element<'a, M, Theme, R>>) -> Self {
        Self {
            keys,
            children,
            width: Length::Shrink,
            height: Length::Shrink,
            spacing: None,
            padding: None,
            max_width: None,
            align_items: None,
        }
    }
    pub(super) fn with_capacity(capacity: usize) -> Self {
        Self::from_vecs(Vec::with_capacity(capacity), Vec::with_capacity(capacity))
    }
    pub(super) fn push(mut self, key: K, child: impl Into<Element<'a, M, Theme, R>>) -> Self {
        let child = child.into();
        let hint = child.as_widget().size_hint();
        // Stock keyed Column, unlike ordinary Row/Column push, has no void filter.
        self.width = self.width.enclose(hint.width);
        self.height = self.height.enclose(hint.height);
        self.keys.push(key);
        self.children.push(child);
        self
    }
    pub(super) fn width(mut self, value: impl Into<Length>) -> Self {
        self.width = value.into();
        self
    }
    pub(super) fn height(mut self, value: impl Into<Length>) -> Self {
        self.height = value.into();
        self
    }
    pub(super) fn spacing(mut self, value: impl Into<Pixels>) -> Self {
        self.spacing = Some(value.into().0);
        self
    }
    pub(super) fn padding(mut self, value: impl Into<Padding>) -> Self {
        self.padding = Some(value.into());
        self
    }
    pub(super) fn max_width(mut self, value: impl Into<Pixels>) -> Self {
        self.max_width = Some(value.into().0);
        self
    }
    pub(super) fn align_items(mut self, value: Alignment) -> Self {
        self.align_items = Some(value);
        self
    }
    /// Native values in child order, not an encoding of application source identity.
    pub fn native_keys(&self) -> &[K] {
        &self.keys
    }
    /// Borrow native children for pre-lowering state preparation.
    ///
    /// ```compile_fail,E0505
    /// use iced_css::{capture::KeyedColumnRecipe,host::{initialize::prepare_native_initialization,reconcile::SlotLimits}};
    /// use iced::advanced::widget::Tree;
    /// fn lower(recipe:KeyedColumnRecipe<'_,u8,(),iced::Theme,()>,slots:&mut [Option<Tree>],limits:SlotLimits) {
    ///     let pending=prepare_native_initialization(slots,recipe.native_children(),limits).unwrap();
    ///     let _=recipe.into_native();
    ///     pending.commit();
    /// }
    /// ```
    pub fn native_children(&self) -> &[Element<'a, M, Theme, R>] {
        &self.children
    }
    /// Raw lifecycle integration only; authored content changes require fresh capture.
    /// Keys and child count remain fixed. Captured exposes no mutable child accessor.
    pub fn native_children_mut(&mut self) -> &mut [Element<'a, M, Theme, R>] {
        &mut self.children
    }
    /// Lower without cloning/comparing keys or replaying native child hints.
    pub fn into_native(self) -> Column<'a, K, M, Theme, R> {
        let mut column = Column::from_vecs(self.keys, self.children)
            .width(self.width)
            .height(self.height);
        if let Some(v) = self.spacing {
            column = column.spacing(v);
        }
        if let Some(v) = self.padding {
            column = column.padding(v);
        }
        if let Some(v) = self.max_width {
            column = column.max_width(v);
        }
        if let Some(v) = self.align_items {
            column = column.align_items(v);
        }
        column
    }
}
impl<'a, K: Copy + PartialEq + 'static, M: 'a, Theme: 'a, R: Renderer + 'a>
    From<KeyedColumnRecipe<'a, K, M, Theme, R>> for Element<'a, M, Theme, R>
{
    fn from(recipe: KeyedColumnRecipe<'a, K, M, Theme, R>) -> Self {
        recipe.into_native().into()
    }
}
