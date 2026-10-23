; L'INSTALLEUR WINDOWS DE PUPPETPAIE (lot I1, 14/10/2026). Se compile par `construire.sh`, jamais seul : il recoit
; VERSION (la version LUE, JJ.MM.AAAA : tout ce que l'installeur montre), VERSION_WINDOWS (l'identifiant AAAA.MM.JJ en a.b.c.0), PREPA (le dossier de preparation, avec poser.nsh, retirer.nsh et pose\), ICONE et SORTIE.
;
; Il pose ce qu'une compagnie recoit -- `logiciel\` et `fictives\` -- dans un dossier ou l'utilisateur ecrit sans etre
; administrateur : `prive\` et `compagnies\` naitront a cote de `logiciel\` (paie/src/interface/compagnies.rs,
; base()). Deux chemins, choisis au debut (Leandre, 14/10/2026) :
;   - PAR DEFAUT : Documents\PuppetPaie (visible, par utilisateur, dans les sauvegardes Windows), un raccourci dans le
;     menu Demarrer, une entree dans « Applications installees » et son desinstalleur ;
;   - PORTABLE : le dossier de son choix (une cle USB) ; le logiciel, ses donnees et la cle qui les scelle y vivent
;     ensemble et s'ouvrent sur n'importe quel ordinateur (`PORTABLE.json` a cote de `logiciel\`, lu par le moteur :
;     paie/src/coffre.rs, dossier_portable). Ni raccourci, ni entree dans Windows, ni desinstalleur : on efface le dossier.
; TOUJOURS BRIDE, sans rien poser pour le dire : sans rien qui dise « debloque », le logiciel est bride (ticket #246,
;   paie/src/bridage.rs). Le geste de l'accueil le debloque (`.edition.json`, a cote de `logiciel\`).
; Le desinstalleur retire ce qu'il a pose, fichier par fichier, et laisse tout le reste.
;
; Les textes lus pour l'ecrire (a deposer, voir prive/journal/finitions/PASSATION-I1.md) : l'exemple NSIS
; `install-per-user.nsi` (installation par utilisateur, cle de desinstallation sous HKCU) ; Microsoft, « Distribute
; your app and the WebView2 Runtime » (detecter le Runtime par la valeur `pv` de deux cles de registre).

!ifndef VERSION
  !error "Se compile par atelier/installeur/construire.sh (VERSION)"
!endif
!ifndef VERSION_WINDOWS
  !error "Se compile par atelier/installeur/construire.sh (VERSION_WINDOWS)"
!endif
!ifndef PREPA
  !error "Se compile par atelier/installeur/construire.sh (PREPA)"
!endif
!ifndef ICONE
  !error "Se compile par atelier/installeur/construire.sh (ICONE)"
!endif
!ifndef SORTIE
  !error "Se compile par atelier/installeur/construire.sh (SORTIE)"
!endif

Unicode True
SetCompressor /SOLID lzma
RequestExecutionLevel user
ManifestDPIAware true

!define NOM "PuppetPaie"
!define CLE_DESINSTALLATION "Software\Microsoft\Windows\CurrentVersion\Uninstall\PuppetPaie"
!define DESINSTALLEUR "desinstaller-PuppetPaie.exe"
!define WEBVIEW2_CLIENT "Microsoft\EdgeUpdate\Clients\{F3017226-FE2A-4295-8BDF-00C3A9A7E4C5}"
!define WEBVIEW2_PAGE "https://developer.microsoft.com/microsoft-edge/webview2/"

Name "${NOM}"
OutFile "${SORTIE}"
BrandingText "${NOM} ${VERSION}"
; Pas de defaut fixe ici : .onInit le pose (une installation existante, sinon Documents\PuppetPaie).
InstallDir ""
InstallDirRegKey HKCU "${CLE_DESINSTALLATION}" "InstallLocation"

VIProductVersion "${VERSION_WINDOWS}"
VIAddVersionKey /LANG=1036 "ProductName" "${NOM}"
VIAddVersionKey /LANG=1036 "FileDescription" "Installeur de ${NOM} ${VERSION}"
VIAddVersionKey /LANG=1036 "ProductVersion" "${VERSION}"
VIAddVersionKey /LANG=1036 "FileVersion" "${VERSION}"
VIAddVersionKey /LANG=1036 "LegalCopyright" "PuppetPaie"

!include MUI2.nsh
!include LogicLib.nsh
!include x64.nsh
!include nsDialogs.nsh

; 1 : installation portable (le choix de la page « Par defaut ou portable »).
Var Portable
Var Dialogue
Var BoutonDefaut
Var BoutonPortable
Var TexteDossier
Var TexteFin

!define MUI_ICON "${ICONE}"
!define MUI_UNICON "${ICONE}"
!define MUI_ABORTWARNING

; --- Les pages de l'installeur ---
!define MUI_WELCOMEPAGE_TITLE "Installer ${NOM} ${VERSION}"
!define MUI_WELCOMEPAGE_TEXT "${NOM} est un logiciel de paie du spectacle vivant.$\r$\n$\r$\nLe logiciel s'installe bridé : l'accueil ne propose que des compagnies fictives, et rien ne part vers un organisme.$\r$\n$\r$\nLe logiciel s'installe pour vous seul, dans un dossier où vous écrivez sans être administrateur : vos données naîtront à côté du programme. La page suivante propose une installation par défaut, dans vos Documents, ou portable, sur une clé USB."
!insertmacro MUI_PAGE_WELCOME

Page custom PageMode PageModeQuitter

!define MUI_DIRECTORYPAGE_TEXT_TOP "$TexteDossier"
!insertmacro MUI_PAGE_DIRECTORY

!insertmacro MUI_PAGE_INSTFILES

!define MUI_FINISHPAGE_TITLE "${NOM} est installé"
!define MUI_FINISHPAGE_TEXT "$TexteFin"
!define MUI_FINISHPAGE_RUN "$INSTDIR\logiciel\puppetpaie.exe"
!define MUI_FINISHPAGE_RUN_TEXT "Ouvrir ${NOM}"
!insertmacro MUI_PAGE_FINISH

; --- Les pages du desinstalleur ---
!define MUI_UNCONFIRMPAGE_TEXT_TOP "Le désinstalleur retire le programme (« logiciel ») et les compagnies fictives (« fictives ») tels qu'ils ont été installés, la mise à jour préparée (« maj »), le raccourci du menu Démarrer, et le choix bridé / débloqué retenu sur ce poste.$\r$\n$\r$\nIl LAISSE vos données : « prive », « compagnies », et tout ce que le logiciel a écrit dans ce dossier."
!insertmacro MUI_UNPAGE_CONFIRM
!insertmacro MUI_UNPAGE_INSTFILES
!define MUI_FINISHPAGE_TITLE "${NOM} est désinstallé"
!define MUI_FINISHPAGE_TEXT "Ce qui n'a pas été installé par l'installeur est resté dans :$\r$\n$INSTDIR$\r$\n$\r$\nVos données y sont toujours ; vous pouvez réinstaller ${NOM} dans ce même dossier pour les retrouver."
!insertmacro MUI_UNPAGE_FINISH

!insertmacro MUI_LANGUAGE "French"

; --- L'installation ---

Function .onInit
  SetShellVarContext current
  ${IfNot} ${RunningX64}
    MessageBox MB_OK|MB_ICONSTOP "${NOM} demande Windows 64 bits."
    Abort
  ${EndIf}
  ${If} $INSTDIR == ""
    StrCpy $INSTDIR "$DOCUMENTS\${NOM}"
  ${EndIf}
  StrCpy $Portable 0
FunctionEnd

; La page « Par defaut ou portable », avant le choix du dossier.
Function PageMode
  !insertmacro MUI_HEADER_TEXT "Par défaut, ou portable" "Où ${NOM} et vos données vont vivre."
  nsDialogs::Create 1018
  Pop $Dialogue
  ${If} $Dialogue == error
    Abort
  ${EndIf}
  ${NSD_CreateRadioButton} 0 0 100% 12u "Par défaut : dans vos Documents (Documents\${NOM})"
  Pop $BoutonDefaut
  ${NSD_AddStyle} $BoutonDefaut ${WS_GROUP}
  ${NSD_CreateLabel} 16u 14u -16u 36u "Un dossier visible, à vous seul, compris dans les sauvegardes de Windows. Un raccourci « ${NOM} » dans le menu Démarrer, et une entrée dans « Applications installées » pour le retirer."
  Pop $0
  ${NSD_CreateRadioButton} 0 56u 100% 12u "Portable : dans le dossier de votre choix (une clé USB)"
  Pop $BoutonPortable
  ${NSD_CreateLabel} 16u 70u -16u 48u "Le logiciel, toutes ses données et la clé qui les scelle vivent ensemble dans ce dossier, qui s'ouvre sur n'importe quel ordinateur. Ni raccourci ni entrée dans Windows : pour le retirer, on efface le dossier. Vos données restent protégées par le mot de passe du logiciel."
  Pop $0
  ${If} $Portable == 1
    ${NSD_Check} $BoutonPortable
    ${NSD_SetFocus} $BoutonPortable
  ${Else}
    ${NSD_Check} $BoutonDefaut
    ${NSD_SetFocus} $BoutonDefaut
  ${EndIf}
  nsDialogs::Show
FunctionEnd

Function PageModeQuitter
  ${NSD_GetState} $BoutonPortable $0
  ${If} $0 == ${BST_CHECKED}
    StrCpy $Portable 1
    StrCpy $TexteDossier "Portable : choisissez le dossier (une clé USB, par exemple). Le programme va dans son sous-dossier « logiciel », les compagnies fictives dans « fictives » ; vos données (« prive », « compagnies ») et la clé qui les scelle naîtront dans ce même dossier."
    StrCpy $TexteFin "${NOM} est installé en portable dans :$\r$\n$INSTDIR$\r$\n$\r$\nIl s'ouvre par logiciel\puppetpaie.exe, sur n'importe quel ordinateur. Pour le retirer, effacez le dossier."
  ${Else}
    StrCpy $Portable 0
    StrCpy $TexteDossier "Le programme va dans le sous-dossier « logiciel », les compagnies fictives dans « fictives ». Vos données (« prive », « compagnies ») naîtront dans ce même dossier : choisissez un dossier où vous avez le droit d'écrire (pas « Program Files »)."
    StrCpy $TexteFin "Le raccourci « ${NOM} » est dans le menu Démarrer.$\r$\n$\r$\nPour le retirer : Paramètres, Applications, ${NOM}. Le désinstalleur laisse vos données."
  ${EndIf}
FunctionEnd

; Le Runtime WebView2 : la valeur `pv` d'une des deux cles que Microsoft documente, plus grande que 0.0.0.0.
Function WebView2Present
  StrCpy $0 ""
  SetRegView 64
  ReadRegStr $0 HKLM "SOFTWARE\WOW6432Node\${WEBVIEW2_CLIENT}" "pv"
  SetRegView 32
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    ReadRegStr $0 HKCU "Software\${WEBVIEW2_CLIENT}" "pv"
  ${EndIf}
FunctionEnd

Section "PuppetPaie" SecProgramme
  SectionIn RO
  SetShellVarContext current

  ; Ce qu'une compagnie recoit : logiciel\ et fictives\, fichier par fichier (liste tiree de Git par construire.sh),
  ; et rien d'autre : bride par defaut, le paquet n'est pas soude (le geste de l'accueil reste possible).
  !include "${PREPA}/poser.nsh"

  SetOutPath "$INSTDIR"
  ${If} $Portable == 1
    ; PORTABLE : le marqueur que le moteur lit (paie/src/coffre.rs, MARQUEUR_PORTABLE) ; la cle du poste
    ; (cle-poste.bin) naitra a cote, dans ce dossier, et non sur la machine. Rien d'autre n'est ecrit hors du dossier.
    FileOpen $1 "$INSTDIR\PORTABLE.json" w
    FileWrite $1 '{"_quoi": "Ce dossier est une installation PORTABLE de PuppetPaie : le logiciel, ses donnees et la cle qui les scelle (cle-poste.bin) vivent ici, ensemble, et s$\'ouvrent sur n$\'importe quel ordinateur. Effacer ce fichier rattache le coffre a la machine."}$\r$\n'
    FileClose $1
    Goto apres_windows
  ${EndIf}

  WriteUninstaller "$INSTDIR\${DESINSTALLEUR}"

  ; Le raccourci du menu Demarrer : le dossier de travail est logiciel\ (SetOutPath juste avant).
  SetOutPath "$INSTDIR\logiciel"
  CreateShortcut "$SMPROGRAMS\${NOM}.lnk" "$INSTDIR\logiciel\puppetpaie.exe" "" "$INSTDIR\logiciel\puppetpaie.exe" 0

  ; « Applications installees » de Windows, pour cet utilisateur.
  WriteRegStr HKCU "${CLE_DESINSTALLATION}" "DisplayName" "${NOM}"
  WriteRegStr HKCU "${CLE_DESINSTALLATION}" "DisplayVersion" "${VERSION}"
  WriteRegStr HKCU "${CLE_DESINSTALLATION}" "Publisher" "PuppetPaie"
  WriteRegStr HKCU "${CLE_DESINSTALLATION}" "DisplayIcon" "$INSTDIR\logiciel\puppetpaie.exe,0"
  WriteRegStr HKCU "${CLE_DESINSTALLATION}" "InstallLocation" "$INSTDIR"
  WriteRegStr HKCU "${CLE_DESINSTALLATION}" "UninstallString" '"$INSTDIR\${DESINSTALLEUR}"'
  WriteRegStr HKCU "${CLE_DESINSTALLATION}" "QuietUninstallString" '"$INSTDIR\${DESINSTALLEUR}" /S'
  WriteRegDWORD HKCU "${CLE_DESINSTALLATION}" "NoModify" 1
  WriteRegDWORD HKCU "${CLE_DESINSTALLATION}" "NoRepair" 1
  apres_windows:

  ; WebView2 absent : l'installeur n'installe rien d'autre que PuppetPaie ; il dit ou Microsoft le donne.
  Call WebView2Present
  ${If} $0 == ""
  ${OrIf} $0 == "0.0.0.0"
    MessageBox MB_YESNO|MB_ICONEXCLAMATION "La fenêtre de ${NOM} s'affiche par « Microsoft Edge WebView2 Runtime », qui semble absent de ce poste (il est livré avec Windows 11 et avec la plupart des Windows 10).$\r$\n$\r$\nMicrosoft le donne ici :$\r$\n${WEBVIEW2_PAGE}$\r$\n$\r$\nOuvrir cette page maintenant ?" /SD IDNO IDNO webview2_fin
    ExecShell "open" "${WEBVIEW2_PAGE}"
  ${EndIf}
  webview2_fin:
SectionEnd

; --- La desinstallation ---

Function un.onInit
  SetShellVarContext current
FunctionEnd

Section "Uninstall"
  SetShellVarContext current

  ; Ce que l'installeur a pose, et rien d'autre ; un dossier qui garde quelque chose reste.
  !include "${PREPA}/retirer.nsh"

  ; Le choix bride / debloque retenu sur ce poste (le geste de l'accueil) : une reinstallation repart bridee.
  Delete "$INSTDIR\.edition.json"
  ; La mise a jour de l'installation (paie/src/maj.rs, lot U1) : son reglage, la version prete, son journal.
  RMDir /r "$INSTDIR\maj"

  Delete "$SMPROGRAMS\${NOM}.lnk"
  DeleteRegKey HKCU "${CLE_DESINSTALLATION}"
  Delete "$INSTDIR\${DESINSTALLEUR}"
  ; Vide, le dossier s'en va ; sinon il reste, avec les donnees.
  RMDir "$INSTDIR"
SectionEnd
