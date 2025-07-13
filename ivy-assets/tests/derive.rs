use ivy_assets::AssetCache;
use ivy_derive::Resource;

#[derive(Debug, Resource)]
pub struct TestResource {
    pub value: u32,
}

#[derive(Debug, Resource)]
pub enum TestResourceEnum {
    Variant1 {
        name: String,
    },
    Variant2 {
        #[resource(load)]
        value: TestResource,
    },
}

#[test]
fn test_resource_derive() {
    futures::executor::block_on(async move {
        let resource = TestResourceDesc { value: 42 };

        let assets = AssetCache::new();

        use ivy_assets::loadable::Loadable;
        let result = resource.load(&assets).await.unwrap();
        eprintln!("Loaded resource: {:?}", result);
    })
}

#[test]
fn test_resource_derive_enum() {
    futures::executor::block_on(async move {
        let resource = vec![
            TestResourceEnumDesc::Variant1 {
                name: "Test Resource".to_string(),
            },
            TestResourceEnumDesc::Variant2 {
                value: TestResourceDesc { value: 42 },
            },
        ];

        let assets = AssetCache::new();

        use ivy_assets::loadable::Loadable;
        let result = resource.load(&assets).await.unwrap();
        eprintln!("Loaded resource: {:#?}", result);
    })
}
