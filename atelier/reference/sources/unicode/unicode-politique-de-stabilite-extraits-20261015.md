# Unicode Character Encoding Stability Policies : extraits

Transcription (le texte de la page HTML, sections citées seulement) de https://www.unicode.org/policies/stability_policy.html,
relevée le 15/10/2026 (HTTP 200, `text/html`, 44 316 octets). La page n'est pas un fichier de données : les conditions
d'Unicode (https://www.unicode.org/copyright.html) n'en permettent pas la diffusion publique. L'original va dans
`reference-privee/sources/unicode/` du dépôt privé ; ici, les passages qui fondent le choix d'une version d'Unicode plus
récente pour les tables du moteur (`LISEZ-MOI.md`).

© Unicode, Inc.

---

Normalization Stability

A. Strong Normalization Stability

Applicable Version: Unicode 4.1+

If a string contains only characters from a given version
of Unicode, and it is put into a normalized form in accordance
with that version of Unicode, then the results will be identical
to the results of putting that string into a normalized form
in accordance with any subsequent version of Unicode.

More formally, given versions V and U of Unicode, and any string
S which only contains characters assigned according to both
V and U, the following are always true:

toNFCV(S) = toNFCU(S)

toNFDV(S) = toNFDU(S)

toNFKCV(S) = toNFKCU(S)

toNFKDV(S) = toNFKDU(S)

In particular, once a character is encoded, its canonical
combining class and decomposition mapping will not be changed
in any way.

Decomposition Mapping

Once a character is assigned, its
decomposition mapping will not change.

Canonical Combining Class

Once a character is assigned, its
canonical combining class will not change.

Note: If an implementation normalizes a string
that contains characters that are not assigned in
the version of Unicode that it supports, that string
might not be in normalized form according to a future
version of Unicode. For example, suppose that a Unicode
5.0 program normalizes a string that contains new Unicode
5.1 characters. That string might not be normalized according
to Unicode 5.1.

B. Weaker Version of Normalization Stability

Applicable Version: Unicode 3.1+

Note:
All of the guarantees implied by this weaker specification are
subsumed by the stricter stability constraints applicable to
Version 4.1 and later.

If a string contains only characters
from a given version of the Unicode, and it is put into a normalized
form in accordance with that version of Unicode, then the result
will also be in that normalized form according to any subsequent
version of Unicode.

The result will also be in
that normalized form according to any prior version of the standard
that contains all of the characters in the string (back to the
first applicable version, Unicode 3.1).

In particular, once a character is encoded, its canonical
combining class and decomposition mapping will not be changed
in a way that will destabilize normalization. Thus the following
constraints will be maintained under all circumstances:

Decomposition Mapping

The decomposition mapping may not
be changed except for the correction of exceptional errors which
meet all of the following conditions (1-3):

There is a clear and evident
error identified in the Unicode Character Database (such
as a typographic mistake).

The error constitutes a clear
violation of the identity stability policy.

The correction of such an error
does not violate the following constraints (a-d):

No character will be given
a decomposition mapping when it did not previously have
one.

No decomposition mapping
will be removed from a character.

No decomposition mapping
will change in type (canonical to compatibility, or
vice versa).

The number of characters
in a decomposition mapping will not change.

Canonical Combining Class

Once a character is assigned, its
canonical combining class will not change.

Note: If an implementation normalizes a string
that contains characters that are not assigned in
the version of Unicode that it supports, that string
might not be in normalized form according to a future
version of Unicode. For example, suppose that a Unicode
4.0 program normalizes a string that contains
new Unicode 4.1 characters. That string might
not be normalized according to Unicode 4.1.

Note: In versions prior to Unicode 4.1,
there were exceptional cases where the normalization algorithm
had to be applied twice to put a string into normalized
form. See

Corrigendum #5: Normalization Idempotency
and
Unicode Standard Annex #15, Unicode Normalization Forms.


[…]

Identity Stability

Applicable Version: Unicode 1.1+

Once a character is encoded, its
properties may still be changed, but not in such a way
as to change the fundamental identity of the character.

The Consortium will endeavor to keep the values of the other
properties as stable as possible, but some circumstances may
arise that require changing them. Particularly in the situation
where the Unicode Standard first encodes less well-documented
characters and scripts, the exact character properties and behavior
initially may not be well known.

As more experience is gathered in implementing the characters,
adjustments in the properties may become necessary. Examples
of such properties include, but are not limited to, the following:

General_Category

Case mappings

Bidirectional properties

Compatibility decomposition tags (such as <font>
or <compat>)


[…]

(Property Value Stability, tableau des invariants : les lignes de la normalisation et des valeurs numériques)

Normalization
Canonical_Combining_Class
The Canonical_Combining_Class property values are limited to the
values 0 to 254.
1.1.5+

Canonical_Combining_Class
Once a character is assigned, its Canonical_Combining_Class will
never change.
3.0.0+

Canonical_Combining_Class, General_Category
All characters other than those with General_Category property
values Spacing_Mark (Mc) and Nonspacing_Mark (Mn) have the
Canonical_Combining_Class property value 0.
1.1.5+

Decomposition_Mapping
Canonical and compatibility mappings (Decomposition_Mapping property
values) are always in canonical order, and the resulting recursive
decomposition will also be in canonical order.
2.0.0+

Decomposition_Mapping
Canonical mappings (Decomposition_Mapping property values) are
always limited either to a single value or to a pair. The second
character in the pair cannot itself have a canonical mapping.
3.0.0+

Decomposition_Mapping
Canonical mappings (Decomposition_Mapping property values) are
always limited so that no string when normalized to NFC expands to more
than 3× in length (measured in code units).
2.0.0+

Decomposition_Mapping
Once a character is assigned, its Decomposition_Mapping will never
change.
4.0.0+

Decomposition_Mapping
Canonical mappings (Decomposition_Mapping property values) to a pair of
characters are limited such that the first of the pair in the mapping must
have ccc=0, except for the Decomposition_Mapping of the following four
characters: U+0344, U+0F73, U+0F75, U+0F81.
2.1.0+

Numeric
General_Category, Numeric_Type
The set of characters having General_Category=Nd will always be the
same as the set of characters having Numeric_Type=de.
4.0.0+

General_Category, Numeric_Type
Characters with the property
value Numeric_Type=de (Decimal) only occur in contiguous ranges of 10
characters, with ascending numeric values from 0 to 9 (Numeric_Value=0..9).
6.0.0+

General_Category, Numeric_Type
Characters with the General_Category property value Number (Nd | Nl | No) do
not have the Numeric_Type value None.
6.2.0+

Numeric_Type
No new assignment of Numeric_Type=di (Digit) will be made to any character.
6.1.0+

