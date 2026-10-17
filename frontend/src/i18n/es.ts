import type { TranslationKey } from './en';

const es: Record<TranslationKey, string> = {
  // Nav
  nav_bounties: 'Recompensas',
  nav_create_bounty: 'Crear recompensa',
  nav_leaderboard: 'Tabla de clasificación',

  // BountyList
  filter_all: 'Todas',
  filter_created_by_me: 'Creadas por mí',
  filter_assigned_to_me: 'Asignadas a mí',
  status_all: 'todas',
  status_open: 'abierta',
  status_claimed: 'reclamada',
  status_disputed: 'en disputa',
  status_completed: 'completada',
  status_cancelled: 'cancelada',
  load_more: 'Cargar más',
  loading: 'Cargando...',

  // BountyDetail (page)
  claiming: 'Reclamando...',
  claim_bounty: 'Reclamar recompensa',

  // CreateBounty (page)
  placeholder_title: 'Título',
  placeholder_description: 'Descripción',
  placeholder_reward: 'Recompensa (XLM)',
  creating: 'Creando...',
  create_bounty: 'Crear recompensa',

  // ContributorProfile
  connect_wallet: 'Conectar billetera',
  connect_wallet_prompt: 'Conecta tu billetera para ver los perfiles de colaboradores.',
  reputation: 'Reputación',
  completed_bounties: 'Recompensas completadas',

  // BountyCard
  loading_bounty: 'Cargando recompensa',

  // BountyErrorBoundary
  error_boundary_message: 'Algo salió mal.',
  retry: 'Reintentar',

  // ShareButton
  share: 'Compartir',
  copy_link: 'Copiar enlace',
  copied: '¡Copiado!',
  copy_failed: 'Error al copiar',

  // CopyButton
  copy_to_clipboard: 'Copiar al portapapeles',

  // Deadline
  deadline_label: 'Fecha límite',
  deadline_urgent: '¡Quedan menos de 24 horas!',
};

export default es;
