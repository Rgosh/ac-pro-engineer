# Car classes, and where their numbers came from

Written 2026-08-17, for v0.4.0. `core/src/games/car_class.rs` is the code; this
is the table with its sources, so a number can be argued with rather than
merely disbelieved.

## Why

Every car was judged against one band: tyres 70–105 °C, brakes 800 °C, taken
from `AlertsConfig::default()`. That band was chosen against Assetto Corsa's
street cars and it is right for them and for almost nothing else.

* A GT3 runs its fronts at **520 °C** all lap. The 800 °C ceiling never fires,
  so the brake advice was silent on the one class where brake temperature is
  the thing people actually manage.
* A Formula car wants its tyres **above 85 °C**. The 70 °C floor called 78 °C
  fine, which is a cold tyre nobody was told about.
* A road car at 520 °C has boiled its fluid, and the same ceiling said nothing.

A threshold that belongs to no car says nothing about any of them. That is most
of the answer to "why is the engineer so quiet".

## The table

| Class | Tyres °C | Front brakes °C | Rear brakes °C | Hot pressure psi |
|---|---|---|---|---|
| Formula | 85–110 | 900 | 800 | 21.0 |
| Prototype | 80–105 | 750 | 600 | 24.0 |
| GTE / GT2 | 80–100 | 700 | 500 | 27.0 |
| **GT3** | **75–100** | **650** | **500** | **27.5** |
| GT4 / Cup | 75–95 | 600 | 450 | 27.0 |
| Touring | 70–95 | 550 | 450 | 28.0 |
| Road / unknown | 70–105 | 500 | 400 | 27.5 |
| Vintage | 60–90 | 400 | 350 | 26.0 |

## Where each came from

**GT3 tyres.** ACC's dry Pirellis are quoted as working between 70 and 100 °C
with peak grip at 80–90; iRacing's GT3s are given the same 80–90 °C. The floor
here is 75 rather than 70 because below that a GT3 is genuinely cold and worth
saying so.

* <https://support.gosetups.gg/hc/en-gb/articles/23222442800018-What-Are-the-Optimal-Tyre-Temperatures-in-ACC>
* <https://cardoracing.com/resources/assetto-corsa-competizione-tires-acc/>
* <https://simracingsetup.com/iracing/iracing-tyre-setup-guide/>

**GT3 brakes.** Common practice is fronts at or under 600–650 °C and rears near
450; real carbon-ceramic discs plateau at 550–750 °C. Hence 650 front and 500
rear — high enough not to cry wolf, low enough to catch a stint that is
cooking.

* <https://www.pitlinelab.com/en/blog/how-to-set-up-brakes-in-assetto-corsa-competizione-to-brake-hard-without-losing-stability>
* <https://thepitcrew.co.uk/coaching/pit-tips/maximize-your-brakes-acc/>
* <https://coachdaveacademy.com/tutorials/brake-pads-in-acc/>

**Formula tyres.** Slicks in the 80–110 °C band, with the harder real-world
compounds higher still. 85 as a floor, 110 as a ceiling.

* <https://www.f1technical.net/forum/viewtopic.php?t=26669>
* <https://f1chronicle.com/understanding-f1-tyre-heat-cycles/>

**GT4 and touring.** Around 80–90 °C for FIA GT cars, on softer rubber and with
less downforce than GT3, so a slightly lower ceiling and a lighter brake load.

* <https://www.yourdatadriven.com/what-should-the-temperature-of-your-racing-car-tyres-be/>

**Road and vintage.** The application's own original band, which was chosen
against Assetto Corsa's street cars, and a cooler one again for cars whose
tyres and brakes are of another era.

**The check that matters.** The recorded Competizione session — a Huracán GT3
EVO at Spa — ran 85–98 °C cores with 520 °C fronts and 257 °C rears for two
laps. `the_recorded_gt3_session_sits_inside_the_gt3_window` asserts every one of
those is inside the GT3 window, because a window that alarms through a normal
stint is worse than no window at all.

## How a class is decided

1. **The game's own tags**, where it has them. Assetto Corsa ships
   `ui_car.json` per car with `gt3`, `#GT4`, `singleseater`, `vintage`,
   `#GTE-GT3` and the rest. That is the game's answer and it wins.
2. **The car's id**, otherwise. Competizione names every car exhaustively —
   `lamborghini_huracan_gt3_evo`, `mercedes_amg_gt4`, `porsche_992_gt3_cup`,
   `bmw_m2_cs_racing` — and Assetto Corsa names most of them well enough. The
   single-seaters are the awkward ones, with no word in common:
   `ks_ferrari_sf70h`, `lotus_exos_125`, `tatuusfa1`, `dallara_f312`.
3. **`Unknown`**, and that is a real answer. A mod nobody has classified keeps
   the driver's own thresholds rather than being pressed into a class it may
   not be in — the same distinction the capability flags draw between "not
   measured" and "measured as zero".

A cup car is read as GT4 rather than GT3 even though its id contains `gt3`:
`porsche_992_gt3_cup` is a one-make car on softer rubber with half the
downforce, and the order of the checks is what gets that right.

## What is still owed

* **The windows are per class, not per car.** A Huracán and a 992 GT3 R are not
  identical, and neither are two Formula cars twenty years apart. Per-car
  numbers would need per-car evidence, and there is none yet — a table of
  guesses at that resolution would read exactly like measurements.
* **Pressures are not used yet.** `hot_pressure_psi` is in the table and the
  cold-pressure calculator still takes the driver's target. Wiring it in means
  deciding what happens when a driver has typed one, and that is the same
  question the temperature band answered: a number somebody set outranks a
  table.
* **Wet compounds.** Every window here is a dry one.
