import React from 'react';
import { Contributor } from '../types';
import { shortenAddress } from '../utils/format';
import { CopyButton } from './CopyButton';

interface ProfileHeaderProps {
  contributor: Contributor;
  /** True when this profile belongs to the connected wallet. */
  isOwn?: boolean;
}

/**
 * Displays the contributor's address and a short "own profile" badge.
 * Part of the redesigned contributor profile (#896).
 */
export function ProfileHeader({ contributor, isOwn = false }: ProfileHeaderProps) {
  return (
    <header className="profile-header">
      <div className="profile-header__avatar" aria-hidden="true">
        {contributor.address.slice(0, 2).toUpperCase()}
      </div>

      <div className="profile-header__info">
        <h1 className="profile-header__address" title={contributor.address}>
          {shortenAddress(contributor.address, 6, 6)}
          <CopyButton value={contributor.address} />
        </h1>

        {isOwn && (
          <span className="profile-header__badge profile-header__badge--own">Your profile</span>
        )}
      </div>
    </header>
  );
}
