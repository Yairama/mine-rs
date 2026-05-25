use std::collections::BTreeMap;

use mine_core::MineError;
use mine_economics::{
    DestinationId, DirectDestinationFeed, MaterialParcel, StockpileDefinition,
    StockpileDegradation, StockpileDeposit, StockpileId, StockpilePeriodInput, StockpilePlanInput,
    StockpileReclaim, StockpileTransactionOrder, evaluate_stockpile_plan,
};

#[test]
fn evaluate_stockpile_balances_and_destination_blending() {
    let plan = StockpilePlanInput::new(
        vec![StockpileDefinition::new(
            StockpileId::new("sp-main").expect("stockpile should be valid"),
            parcel(100.0, &[("cu", 1.0)]),
            None,
        )],
        vec![
            StockpilePeriodInput::new(
                "P1",
                vec![
                    StockpileDeposit::new(
                        StockpileId::new("sp-main").expect("stockpile should be valid"),
                        parcel(50.0, &[("cu", 1.5)]),
                    )
                    .expect("deposit should be valid"),
                ],
                vec![
                    StockpileReclaim::new(
                        StockpileId::new("sp-main").expect("stockpile should be valid"),
                        DestinationId::new("mill").expect("destination should be valid"),
                        60.0,
                    )
                    .expect("reclaim should be valid"),
                ],
                vec![
                    DirectDestinationFeed::new(
                        DestinationId::new("mill").expect("destination should be valid"),
                        parcel(30.0, &[("cu", 0.9)]),
                    )
                    .expect("direct feed should be valid"),
                ],
            )
            .expect("period should be valid"),
        ],
        StockpileTransactionOrder::DepositThenReclaim,
    )
    .expect("plan should be valid");

    let report = evaluate_stockpile_plan(&plan).expect("plan should evaluate");
    let period = &report.periods[0];
    let stockpile = &period.stockpile_balances[0];
    let mill = &period.destination_blends[0];

    assert_eq!(stockpile.opening_balance.tonnes(), 100.0);
    assert_eq!(stockpile.deposited_material.tonnes(), 50.0);
    assert_eq!(stockpile.reclaimed_material.tonnes(), 60.0);
    assert_close(stockpile.reclaimed_material.contained_metals()["cu"], 1.0);
    assert_eq!(stockpile.closing_balance.tonnes(), 90.0);
    assert_close(stockpile.closing_balance.contained_metals()["cu"], 1.5);

    assert_eq!(mill.destination_id.as_str(), "mill");
    assert_eq!(mill.blended_feed.tonnes(), 90.0);
    assert_close(mill.blended_feed.contained_metals()["cu"], 1.9);
    assert_close(mill.blended_grades["cu"], 1.9 / 90.0);
    assert_eq!(mill.contributing_stockpiles.len(), 1);
    assert_eq!(mill.contributing_stockpiles[0].as_str(), "sp-main");
}

#[test]
fn apply_degradation_before_period_movements() {
    let plan = StockpilePlanInput::new(
        vec![StockpileDefinition::new(
            StockpileId::new("sp-weathered").expect("stockpile should be valid"),
            parcel(100.0, &[("cu", 2.0)]),
            Some(StockpileDegradation::new(0.1, 0.2).expect("degradation should be valid")),
        )],
        vec![
            StockpilePeriodInput::new(
                "P1",
                vec![],
                vec![
                    StockpileReclaim::new(
                        StockpileId::new("sp-weathered").expect("stockpile should be valid"),
                        DestinationId::new("mill").expect("destination should be valid"),
                        45.0,
                    )
                    .expect("reclaim should be valid"),
                ],
                vec![],
            )
            .expect("period should be valid"),
        ],
        StockpileTransactionOrder::ReclaimThenDeposit,
    )
    .expect("plan should be valid");

    let report = evaluate_stockpile_plan(&plan).expect("plan should evaluate");
    let stockpile = &report.periods[0].stockpile_balances[0];

    assert_eq!(stockpile.degraded_material.tonnes(), 10.0);
    assert_close(stockpile.degraded_material.contained_metals()["cu"], 0.4);
    assert_eq!(stockpile.reclaimed_material.tonnes(), 45.0);
    assert_close(stockpile.reclaimed_material.contained_metals()["cu"], 0.8);
    assert_eq!(stockpile.closing_balance.tonnes(), 45.0);
    assert_close(stockpile.closing_balance.contained_metals()["cu"], 0.8);
}

#[test]
fn explicit_transaction_order_changes_reclaim_composition() {
    let deposit_then_reclaim = evaluate_stockpile_plan(
        &StockpilePlanInput::new(
            vec![StockpileDefinition::new(
                StockpileId::new("sp-order").expect("stockpile should be valid"),
                parcel(100.0, &[("cu", 1.0)]),
                None,
            )],
            vec![
                StockpilePeriodInput::new(
                    "P1",
                    vec![
                        StockpileDeposit::new(
                            StockpileId::new("sp-order").expect("stockpile should be valid"),
                            parcel(100.0, &[("cu", 4.0)]),
                        )
                        .expect("deposit should be valid"),
                    ],
                    vec![
                        StockpileReclaim::new(
                            StockpileId::new("sp-order").expect("stockpile should be valid"),
                            DestinationId::new("mill").expect("destination should be valid"),
                            100.0,
                        )
                        .expect("reclaim should be valid"),
                    ],
                    vec![],
                )
                .expect("period should be valid"),
            ],
            StockpileTransactionOrder::DepositThenReclaim,
        )
        .expect("plan should be valid"),
    )
    .expect("plan should evaluate");

    let reclaim_then_deposit = evaluate_stockpile_plan(
        &StockpilePlanInput::new(
            vec![StockpileDefinition::new(
                StockpileId::new("sp-order").expect("stockpile should be valid"),
                parcel(100.0, &[("cu", 1.0)]),
                None,
            )],
            vec![
                StockpilePeriodInput::new(
                    "P1",
                    vec![
                        StockpileDeposit::new(
                            StockpileId::new("sp-order").expect("stockpile should be valid"),
                            parcel(100.0, &[("cu", 4.0)]),
                        )
                        .expect("deposit should be valid"),
                    ],
                    vec![
                        StockpileReclaim::new(
                            StockpileId::new("sp-order").expect("stockpile should be valid"),
                            DestinationId::new("mill").expect("destination should be valid"),
                            100.0,
                        )
                        .expect("reclaim should be valid"),
                    ],
                    vec![],
                )
                .expect("period should be valid"),
            ],
            StockpileTransactionOrder::ReclaimThenDeposit,
        )
        .expect("plan should be valid"),
    )
    .expect("plan should evaluate");

    let reclaim_before = &deposit_then_reclaim.periods[0].stockpile_balances[0].reclaimed_material;
    let reclaim_after = &reclaim_then_deposit.periods[0].stockpile_balances[0].reclaimed_material;

    assert_close(reclaim_before.contained_metals()["cu"], 2.5);
    assert_close(reclaim_after.contained_metals()["cu"], 1.0);
}

#[test]
fn reject_reclaim_above_available_balance() {
    let plan = StockpilePlanInput::new(
        vec![StockpileDefinition::new(
            StockpileId::new("sp-limited").expect("stockpile should be valid"),
            parcel(50.0, &[("cu", 1.0)]),
            None,
        )],
        vec![
            StockpilePeriodInput::new(
                "P1",
                vec![],
                vec![
                    StockpileReclaim::new(
                        StockpileId::new("sp-limited").expect("stockpile should be valid"),
                        DestinationId::new("mill").expect("destination should be valid"),
                        60.0,
                    )
                    .expect("reclaim should be valid"),
                ],
                vec![],
            )
            .expect("period should be valid"),
        ],
        StockpileTransactionOrder::ReclaimThenDeposit,
    )
    .expect("plan should be valid");

    let error = evaluate_stockpile_plan(&plan).expect_err("reclaim should fail");
    assert_eq!(
        error,
        MineError::Economics {
            message: "cannot reclaim 60.000000 t from a balance with only 50.000000 t available"
                .to_owned(),
        }
    );
}

#[test]
fn reject_unknown_stockpile_reference() {
    let error = StockpilePlanInput::new(
        vec![StockpileDefinition::new(
            StockpileId::new("sp-main").expect("stockpile should be valid"),
            parcel(10.0, &[("cu", 0.2)]),
            None,
        )],
        vec![
            StockpilePeriodInput::new(
                "P1",
                vec![
                    StockpileDeposit::new(
                        StockpileId::new("sp-missing").expect("stockpile should be valid"),
                        parcel(5.0, &[("cu", 0.1)]),
                    )
                    .expect("deposit should be valid"),
                ],
                vec![],
                vec![],
            )
            .expect("period should be valid"),
        ],
        StockpileTransactionOrder::DepositThenReclaim,
    )
    .expect_err("unknown stockpile should fail");

    assert_eq!(
        error,
        MineError::Validation {
            message: "period `P1` references unknown stockpile `sp-missing` in deposit".to_owned(),
        }
    );
}

fn parcel(tonnes: f64, metals: &[(&str, f64)]) -> MaterialParcel {
    MaterialParcel::new(
        tonnes,
        metals
            .iter()
            .map(|(metal, quantity)| ((*metal).to_owned(), *quantity))
            .collect::<BTreeMap<_, _>>(),
    )
    .expect("parcel should be valid")
}

fn assert_close(actual: f64, expected: f64) {
    assert!(
        (actual - expected).abs() < 1e-9,
        "expected {expected}, got {actual}"
    );
}
