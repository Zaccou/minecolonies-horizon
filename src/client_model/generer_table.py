#!/usr/bin/env python3
"""Genere `table.rs` : la table des 95 fichiers de blockstates du mod, mesuree.

Regle : **rien n'est recopie a l'oeil**. Chaque ligne vient du JSON du mod
(`assets/minecolonies/blockstates/*.json`) ; le cote Java n'apporte que ce que le
JSON ne dit pas - la classe qui declare les proprietes, et le domaine de chacune -
et il est ecrit ici avec son `fichier.java:ligne`.

Deux empreintes FNV-1a 64 sont emises : une par ligne et une globale. Le test
`variants::tests::la_table_porte_l_empreinte_des_json` les recalcule depuis la
table Rust ; si une ligne bouge, il tombe. C'est ce qui empeche la table de
deriver du JSON sans que personne ne le voie.

Usage :
    python3 src/client_model/generer_table.py            # ecrit table.rs
    python3 src/client_model/generer_table.py --audit    # imprime l'audit
"""

import json
import os
import shutil
import subprocess
import sys

RACINE = os.path.dirname(os.path.abspath(__file__))
MOD = os.path.abspath(os.path.join(
    RACINE, "..", "..", "..", "minecolonies-src", "src", "main", "resources",
    "assets", "minecolonies"))

# --- domaines de proprietes, releves dans le Java -----------------------------
# Chaque entree : (nom, valeurs dans l ordre de l enum / de la propriete).
FACING4 = ["north", "east", "south", "west"]                # HorizontalDirectionalBlock.FACING
DIR6 = ["down", "up", "north", "south", "west", "east"]     # Direction.values()
BOOL2 = ["true", "false"]                                   # BooleanProperty.getPossibleValues
MOISTURE8 = [str(i) for i in range(8)]                      # BlockStateProperties.MOISTURE
AGE7 = [str(i) for i in range(7)]                           # IntegerProperty.create("age",0,6)
ROT16 = [str(i) for i in range(16)]                         # BlockStateProperties.ROTATION_16
HINGE2 = ["left", "right"]                                  # DoorBlock.HINGE / DoorHingeSide
HALF2 = ["upper", "lower"]                                  # DOUBLE_BLOCK_HALF
GRAVE2 = ["blockgravedefault", "blockgravedecayed"]         # api/blocks/types/GraveType.java:11-12
# api/blocks/types/RackType.java:11-15, dans l ordre de declaration
RACK5 = ["blockrackemptysingle", "blockrackfullsingle", "blockrackempty", "blockrackfull", "blockrackair"]
# api/blocks/types/BarrelType.java:11-18 (le nom serialise est la 2e colonne)
BARREL8 = ["0perc", "20perc", "40perc", "60perc", "80perc", "100perc", "working", "done"]

P = lambda nom, valeurs: (nom, valeurs)

# --- proprietes declarees, par bloc -------------------------------------------
# La source est la ligne du `builder.add(...)` qui enregistre la definition d etat,
# ou la ligne du champ quand le bloc n a pas de definition propre (une seule etat).
HUT = ("AbstractColonyBlock.java:281", [P("facing", FACING4)])
HUTS = {}


def hut(noms):
    for n in noms:
        HUTS[n] = HUT


decl = {
    "barrel_block": ("BlockBarrel.java:63", [P("facing", FACING4), P("variant", BARREL8)]),
    "blockconstructiontape": ("BlockConstructionTape.java:251",
                              [P("north", BOOL2), P("east", BOOL2), P("south", BOOL2),
                               P("west", BOOL2), P("facing", FACING4),
                               P("corner", BOOL2), P("waterlogged", BOOL2)]),
    "blockhutfield": ("AbstractBlockMinecoloniesDefault.java:44", [P("facing", FACING4)]),
    "blockhutplantationfield": ("BlockPlantationField.java:229",
                                [P("facing", FACING4), P("mirror", BOOL2)]),
    "blockminecoloniesgrave": ("BlockMinecoloniesGrave.java:193",
                               [P("facing", FACING4), P("variant", GRAVE2)]),
    "blockminecoloniesnamedgrave": ("BlockMinecoloniesNamedGrave.java:77", [P("facing", FACING4)]),
    "blockminecoloniesrack": ("BlockMinecoloniesRack.java:250",
                              [P("facing", FACING4), P("variant", RACK5)]),
    "blockwaypoint": ("BlockWaypoint.java:12", []),
    "colony_banner": ("BlockColonyFlagBanner.java:80", [P("rotation", ROT16)]),
    "colony_wall_banner": ("BlockColonyFlagWallBanner.java:103", [P("facing", FACING4)]),
    "colonysign": ("BlockColonySign.java:141", [P("facing", FACING4), P("connected", BOOL2)]),
    "composted_dirt": ("BlockCompostedDirt.java:26", []),
    "decorationcontroller": ("BlockDecorationController.java:206",
                             [P("facing", DIR6), P("mirror", BOOL2), P("waterlogged", BOOL2)]),
    "farmland": ("MinecoloniesFarmland.java:222", [P("moisture", MOISTURE8), P("waterlogged", BOOL2)]),
    "floodedfarmland": ("MinecoloniesFarmland.java:222", [P("moisture", MOISTURE8), P("waterlogged", BOOL2)]),
    "gate_iron": ("AbstractBlockGate.java:40", [P("facing", FACING4), P("hinge", HINGE2),
                                                P("open", BOOL2), P("powered", BOOL2), P("half", HALF2)]),
    "gate_wood": ("AbstractBlockGate.java:40", [P("facing", FACING4), P("hinge", HINGE2),
                                                P("open", BOOL2), P("powered", BOOL2), P("half", HALF2)]),
}
for CROP in ["bell_pepper", "butternut_squash", "cabbage", "chickpea", "cotton", "corn", "durum",
             "eggplant", "garlic", "mint", "nether_pepper", "onion", "peas", "rice", "soybean",
             "tomato"]:
    decl[CROP] = ("MinecoloniesCropBlock.java:198", [P("age", AGE7)])

# Les 52 huttes + les 3 carrieres + les 2 blocs de colonie partagent la definition d etat.
HUT_BLOCKS = [n[:-5] for n in sorted(os.listdir(os.path.join(MOD, "blockstates")))
              if n.startswith("blockhut")]
HUTS_BLOCKS = HUT_BLOCKS + ["largequarry", "mediumquarry", "simplequarry", "blockpostbox", "blockstash"]
for B in HUTS_BLOCKS:
    if B not in decl and B not in ("blockhutfield", "blockhutplantationfield"):
        decl[B] = HUT

# --- blocs qui n existent plus cote Java --------------------------------------
# (raison, fichier.java:ligne de la preuve)
ORPHELINS = {
    "largequarry": "enregistrement commente, ModBlocksInitializer.java:151",
    "cotton": "aucune classe Java ne declare ce bloc",
}

# --- fichiers qui ne sont pas des blockstates --------------------------------
# Ce sont des modeles d objet ranges dans `blockstates/` : vanilla les refuse
# (« no variants or multipart »).
ITEMS = ["caliper", "compost", "sceptergold", "scepterpermission", "sceptersteel",
         "supplycamp", "supplychest"]
for ITEM in ITEMS:
    decl[ITEM] = ("modele d objet range dans blockstates/", [])

CONV_A = {"north": 0, "east": 1, "south": 2, "west": 3}
CONV_B = {"north": 3, "east": 0, "south": 1, "west": 2}
CONV_C = {"north": 1, "east": 2, "south": 3, "west": 0}
CONV_D = {"north": 2, "east": 3, "south": 0, "west": 1}


def fnv1a64(octets):
    h = 0xCBF29CE484222325
    for b in octets:
        h = ((h ^ b) * 0x100000001B3) & 0xFFFFFFFFFFFFFFFF
    return h


def appliques(valeur):
    return valeur if isinstance(valeur, list) else [valeur]


def lire(nom):
    with open(os.path.join(MOD, "blockstates", nom + ".json"), encoding="utf-8") as f:
        return json.load(f)


def mesure(nom):
    d = lire(nom)
    if "variants" not in d and "multipart" not in d:
        return dict(kind="ItemModel", selectors=0, keys=[], models=[], turns=[], uvlock=False,
                    weights=[], named=[])
    modeles, tours, poids, cles, nommees = set(), set(), set(), [], set()
    uv = False
    if "variants" in d:
        kind = "Variants"
        for cle, valeur in d["variants"].items():
            cles.append(cle)
            for a in appliques(valeur):
                modeles.add(a["model"])
                tours.add((a.get("x", 0) // 90, a.get("y", 0) // 90))
                uv = uv or a.get("uvlock", False)
                poids.add(a.get("weight", 1))
            if cle:
                for clause in cle.split(","):
                    nommees.add(clause.split("=")[0])
    else:
        kind = "Multipart"
        for cas in d["multipart"]:
            for a in appliques(cas["apply"]):
                modeles.add(a["model"])
                tours.add((a.get("x", 0) // 90, a.get("y", 0) // 90))
                uv = uv or a.get("uvlock", False)
                poids.add(a.get("weight", 1))
            quand = cas.get("when", {})
            for cle in quand:
                if cle not in ("OR", "AND"):
                    nommees.add(cle)
    return dict(kind=kind, selectors=len(cles) if kind == "Variants" else len(d["multipart"]),
                keys=cles, models=sorted(modeles), turns=sorted(tours), uvlock=uv,
                weights=sorted(poids), named=sorted(nommees))


def convention(nom, m):
    """Le mapping `facing` -> quart de tour, quand la famille en porte un."""
    if m["kind"] != "Variants":
        return "None"
    table = {}
    for cle, valeur in lire(nom)["variants"].items():
        morceaux = dict(c.split("=", 1) for c in cle.split(",") if "=" in c)
        if "facing" not in morceaux:
            return "None"
        for a in appliques(valeur):
            table.setdefault(morceaux["facing"], set()).add(a.get("y", 0) // 90)
    if set(table) != set(CONV_A):
        return "Vertical" if "up" in table or "down" in table else "None"
    if any(len(v) != 1 for v in table.values()):
        # `facing` tourne, mais une variante de `variant` tourne d un autre cran :
        # le rack, dont `blockrackair` est a 180 degres des quatre autres.
        return "PerVariant"
    table = {k: next(iter(v)) for k, v in table.items()}
    for nom_conv, conv in (("A", CONV_A), ("B", CONV_B), ("C", CONV_C), ("D", CONV_D)):
        if table == conv:
            return nom_conv
    if table == {"north": 0, "east": 1, "south": 0, "west": 1}:
        return "Folded"
    return "None"


CONSTANTES = {
    tuple(FACING4): "FACING4",
    tuple(DIR6): "DIR6",
    tuple(BOOL2): "BOOL2",
    tuple(MOISTURE8): "MOISTURE8",
    tuple(AGE7): "AGE7",
    tuple(ROT16): "ROT16",
    tuple(HINGE2): "HINGE2",
    tuple(HALF2): "HALF2",
    tuple(GRAVE2): "GRAVE2",
    tuple(RACK5): "RACK5",
    tuple(BARREL8): "BARREL8",
}


def domaine(valeurs):
    nom = CONSTANTES.get(tuple(valeurs))
    if nom:
        return nom
    return "&[%s]" % ", ".join('"%s"' % v for v in valeurs)


def etats(props):
    """Tous les etats possibles, en produit cartesien des domaines."""
    resultat = [{}]
    for nom, valeurs in props:
        resultat = [dict(e, **{nom: v}) for e in resultat for v in valeurs]
    return resultat


def clause_matche(clause, etat):
    k, v = clause.split("=", 1)
    return etat.get(k) in v.split("|")


def couverture(nom, m, props):
    """Nombre d etats qu au moins un selecteur du fichier nomme."""
    if m["kind"] == "ItemModel":
        return 0
    tous = etats(props)
    if m["kind"] == "Multipart":
        d = lire(nom)
        return sum(1 for e in tous if any("when" not in cas for cas in d["multipart"]))
    d = lire(nom)
    n = 0
    for e in tous:
        for cle in d["variants"]:
            if cle == "" or all(clause_matche(c, e) for c in cle.split(",")):
                n += 1
                break
    return n


def texte_canonique(nom, m):
    return "|".join([
        nom, m["kind"], ",".join(m["named"]), str(m["selectors"]), str(m["couvert"]),
        ",".join(x.split(":", 1)[-1] for x in m["models"]),
        ",".join("%d:%d" % t for t in m["turns"]), "true" if m["uvlock"] else "false",
        ",".join(str(w) for w in m["weights"]),
    ])


def ligne(nom, m, d):
    print_ = fnv1a64(texte_canonique(nom, m).encode())
    if m["kind"] == "ItemModel":
        return """    Family {
        block: "%s", kind: Kind::ItemModel, props: &[], named: &[], selectors: 0, covered: 0, models: &[],
        turns: &[], uvlock: false, weights: &[], conv: Conv::None, source: "%s",
        standing: Standing::ItemModel, print: 0x%016x,
    },""" % (nom, d[0], print_)
    classe, props = d
    orphelin = ORPHELINS.get(nom)
    standing = ("Standing::Orphan { why: \"%s\" }" % orphelin) if orphelin else "Standing::Registered"
    props_rs = ", ".join('p("%s", %s)' % (k, domaine(vs)) for k, vs in props)
    return """    Family {
        block: "%s", kind: Kind::%s, props: &[%s],
        named: &[%s], selectors: %d, covered: %d,
        models: &[%s],
        turns: &[%s], uvlock: %s, weights: &[%s], conv: Conv::%s,
        source: "%s", standing: %s, print: 0x%016x,
    },""" % (
        nom, m["kind"], props_rs,
        ", ".join('"%s"' % n for n in m["named"]), m["selectors"], m["couvert"],
        ", ".join('"%s"' % x.split(":", 1)[-1] for x in m["models"]),
        ", ".join("(%d, %d)" % t for t in m["turns"]), "true" if m["uvlock"] else "false",
        ", ".join(str(w) for w in m["weights"]), convention(nom, m), classe, standing, print_)


def load(chemin):
    with open(chemin, encoding="utf-8") as f:
        return json.load(f)


def audit_modeles():
    """Les nombres cites par `transforms.rs` et `textures.rs`, mesures ici.

    Parcourt `models/block/**.json` et `models/item/*.json` : elements, rotations
    d element, entrees `display`, references de texture.
    """
    from collections import Counter
    blocs, items = [], []
    base = os.path.join(MOD, "models")
    for racine, _, fichiers in os.walk(os.path.join(base, "block")):
        blocs.extend(os.path.join(racine, f) for f in fichiers)
    items = [os.path.join(base, "item", f) for f in sorted(os.listdir(os.path.join(base, "item")))]
    elements = rotationes = y_zero = 0
    angles, axes = Counter(), Counter()
    rescale = 0
    refs, distinct = 0, set()
    propres = 0
    references = 0
    for chemin in blocs + items:
        d = load(chemin)
        for valeur in d.get("textures", {}).values():
            if not isinstance(valeur, str):
                continue
            if valeur.startswith("#"):
                references += 1
                continue
            if valeur.startswith("minecolonies:"):
                propres += 1
                continue
            refs += 1
            distinct.add(valeur)
        for element in d.get("elements", []):
            elements += 1
            rotation = element.get("rotation")
            if not rotation:
                continue
            rotationes += 1
            angles[rotation.get("angle")] += 1
            axes[rotation.get("axis")] += 1
            if rotation.get("axis") == "y" and rotation.get("angle") == 0:
                y_zero += 1
            if rotation.get("rescale"):
                rescale += 1
    entetes = Counter()
    for chemin in items:
        entetes[load(chemin).get("parent", "<none>")] += 1
    display = 0
    legacy = 0
    for chemin in items:
        noms = set(load(chemin).get("display", {}))
        if noms & {"thirdperson", "firstperson"}:
            legacy += 1
        display += len(noms)
    print("modeles de bloc          :", len(blocs))
    print("modeles d objet          :", len(items))
    print("elements                 :", elements)
    print("elements avec rotation   :", rotationes)
    print("  dont axis=y angle=0    :", y_zero)
    print("  dont rescale           :", rescale)
    print("angles                   :", dict(sorted(angles.items())))
    print("axes                     :", dict(axes))
    print("references de texture    :", refs + propres + references)
    print("  vers vanilla           :", refs, "dont", len(distinct), "noms distincts")
    print("  vers minecolonies      :", propres)
    print("  references #nom        :", references)
    print("entrees display (items)  :", display)
    print("items a contexte ancien  :", legacy)
    print("parents d item           :", dict(entetes.most_common(6)))


def main():
    if "--audit-modeles" in sys.argv:
        audit_modeles()
        return
    noms = [n[:-5] for n in sorted(os.listdir(os.path.join(MOD, "blockstates")))]
    mesures = {n: dict(mesure(n), couvert=couverture(n, mesure(n), decl[n][1])) for n in noms}
    lignes = [ligne(n, mesures[n], decl[n]) for n in noms]
    global_ = fnv1a64("\n".join(texte_canonique(n, mesures[n]) for n in noms).encode())

    if "--audit" in sys.argv:
        from collections import Counter
        print("blockstates :", len(noms))
        print("genres      :", dict(Counter(m["kind"] for m in mesures.values())))
        print("conventions :", dict(Counter(convention(n, mesures[n]) for n in noms if mesures[n]["kind"] == "Variants")))
        for n in noms:
            m, d = mesures[n], decl[n]
            if m["kind"] == "ItemModel":
                continue
            total = 1
            for _, vs in d[1]:
                total *= len(vs)
            couverts = 1
            for k, vs in d[1]:
                if k not in m["named"]:
                    couverts *= len(vs)
            if m["kind"] == "Multipart":
                # Un multipart couvre tous les etats : un cas sans `when` s'applique
                # toujours (ici `blockconstructiontape_center`).
                couverts = total
            etats = 0
            if m["kind"] == "Variants":
                prof = {}
                for cle in m["keys"]:
                    for c in cle.split(","):
                        if "=" in c:
                            k, v = c.split("=", 1)
                            prof.setdefault(k, set()).add(v)
                etats = 1
                for _, vs in d[1]:
                    etats *= len(vs)
                manquants = [k for k, vs in d[1] if k in m["named"] and prof.get(k) != set(vs)]
                couverts = 1
                for k, vs in d[1]:
                    if k in prof:
                        couverts *= len(prof[k])
                    else:
                        couverts *= len(vs)
            print("%-30s %-9s selecteurs=%-3d etats=%-4d couverts=%-4d nommees=%-22s %s" % (
                n, m["kind"], m["selectors"], total, couverts, ",".join(m["named"]) or "-",
                d[0]))
        print("empreinte globale : 0x%016x" % global_)
        return

    entete = '''//! La table des 95 fichiers de `assets/minecolonies/blockstates/`, mesuree.
//!
//! **Genere** par `src/client_model/generer_table.py` a partir des JSON du mod :
//! aucun nom de modele, aucun quart de tour et aucun poids n'est ecrit a la main.
//! Le champ `source` est la seule partie qui vient du Java (la classe qui declare
//! les proprietes), et le script l'ecrit avec son `fichier.java:ligne`.
//!
//! `print` est l'empreinte FNV-1a 64 (`0xcbf29ce484222325`, prime
//! `0x100000001b3`) de la ligne canonique
//! `<bloc>|<genre>|<proprietes nommees>|<selecteurs>|<modeles>|<tours>|<uvlock>|<poids>`,
//! modeles sans leur espace de noms, tours en quarts de tour. `variants.rs` la
//! recalcule et refuse une table qui a derive.
//!
//! Ne pas editer : relancer le script.

use crate::client_model::variants::*;

/// Les 95 fichiers, dans l'ordre alphabetique de leur nom (celui du fichier).
pub const FAMILIES: &[Family] = &[
'''
    pied = "];\n\n/// L'empreinte FNV-1a 64 des 95 lignes ci-dessus, dans ce meme ordre.\npub const FINGERPRINT: u64 = 0x%016x;\n" % global_
    sortie = os.path.join(RACINE, "table.rs")
    with open(sortie, "w", encoding="utf-8") as f:
        f.write(entete + "\n".join(lignes) + "\n" + pied)
    # Le fichier est genere, donc il doit sortir deja formate : sans cela
    # `cargo fmt --check` refuserait la table et un humain serait tente de la
    # reformater a la main.
    if shutil.which("rustfmt"):
        subprocess.run(
            [shutil.which("rustfmt"), "--edition", "2024", sortie],
            check=False,
        )
    print("ecrit", sortie, len(lignes), "lignes, empreinte 0x%016x" % global_)


if __name__ == "__main__":
    main()
