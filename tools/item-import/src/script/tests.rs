use super::*;

#[test]
fn shapes() {
    let heal = |hp, sp| Some(ItemUse::ItemHeal { hp, sp });
    assert_eq!(lift("itemheal rand(10,20),0;\n"), heal((10, 20), (0, 0)));
    assert_eq!(lift("itemheal 7,rand(3,4);"), heal((7, 7), (3, 4)));
    assert_eq!(lift("itemheal rand(1,2),rand(3,4);"), heal((1, 2), (3, 4)));
    assert_eq!(lift(" itemheal  5 , 6 ;"), heal((5, 5), (6, 6)));
    assert_eq!(lift("itemheal 9,0; /* note */"), heal((9, 9), (0, 0)));
    assert_eq!(lift("/* todo */\nitemheal 9,0;"), heal((9, 9), (0, 0)));
    assert_eq!(
        lift("percentheal 25,10; // x"),
        Some(ItemUse::PercentHeal { hp: 25, sp: 10 })
    );
}

#[test]
fn not_lifted() {
    for script in [
        "itemheal 5,0; sc_end SC_POISON;",
        "percentheal 5,0; itemheal 0,5;",
        "if (x) itemheal 5,0;",
        "itemheal -5,0;",
        "itemheal rand(9,1),0;",
        "itemheal 5,0,getcharid(0);",
        "itemheal .@a,0;",
        "itemheal 5,0",
        "percentheal 300,0;",
        "getitem 501,1;",
        "",
    ] {
        assert_eq!(lift(script), None, "{script:?}");
    }
}
