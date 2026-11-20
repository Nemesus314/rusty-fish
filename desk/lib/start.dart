/// Womit das Fenster aufgehen soll, wenn es nicht die bloße Übersicht ist.
///
/// - `--edit NAME`: das Profil gleich zum Bearbeiten öffnen
/// - `--edit NAME --at rooms` oder `--at people`: das Formular bei diesem
///   Abschnitt öffnen
/// - `--new`: ein neues Profil anlegen
/// - `--secrets`: die Geheimnisse öffnen
///
/// Gedacht für Verknüpfungen, und damit sich jede Ansicht des echten Fensters
/// ansehen lässt, ohne zu klicken. `--at` kam am 20.11.2026 dazu: die unteren
/// Abschnitte des Formulars lagen sonst immer unter dem sichtbaren Bereich.
class StartRequest {
  const StartRequest.edit(String this.profile, {this.section})
    : newProfile = false,
      secrets = false;
  const StartRequest.newProfile()
    : profile = null,
      section = null,
      newProfile = true,
      secrets = false;
  const StartRequest.secrets()
    : profile = null,
      section = null,
      newProfile = false,
      secrets = true;

  final String? profile;

  /// Der Abschnitt des Formulars, bei dem es aufgehen soll.
  final String? section;
  final bool newProfile;
  final bool secrets;

  static StartRequest? fromArgs(List<String> args) {
    for (final (index, arg) in args.indexed) {
      if (arg == '--new') {
        return const StartRequest.newProfile();
      }
      if (arg == '--secrets') {
        return const StartRequest.secrets();
      }
      if (arg == '--edit' && index + 1 < args.length) {
        final at = args.indexOf('--at');
        return StartRequest.edit(
          args[index + 1],
          section: at >= 0 && at + 1 < args.length ? args[at + 1] : null,
        );
      }
    }
    return null;
  }
}
