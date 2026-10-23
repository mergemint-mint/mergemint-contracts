import React, { useEffect } from 'react';
import { BrowserRouter, Link, Route, Routes, useLocation } from 'react-router-dom';
import { WalletProvider, useWallet } from './lib/WalletContext';
import { WalletConnectButton } from './components/WalletConnectButton';
import { NetworkMismatchBanner } from './components/NetworkMismatchBanner';
import { BountyList } from './pages/BountyList';
import { BountyDetail } from './pages/BountyDetail';
import { CreateBounty } from './pages/CreateBounty';
import { ContributorProfile } from './pages/ContributorProfile';
import { Leaderboard } from './pages/Leaderboard';
import { Stats } from './pages/Stats';
import { Admin } from './pages/Admin';
import { NotFound } from './pages/NotFound';
import MyBounties from './pages/MyBounties';
import { BountyErrorBoundary } from './components/BountyErrorBoundary';
import { useTranslation } from './i18n';

function Nav() {
  const location = useLocation();
  const { address, connect, clearError, error } = useWallet();
  const { t } = useTranslation();

  // A prior connect() failure otherwise stays visible until the next
  // connect() attempt, even after navigating away (issue #508).
  useEffect(() => {
    clearError();
  }, [location.pathname, clearError]);

  return (
    <nav>
      <Link to="/">{t('nav_bounties')}</Link>
      <Link to="/create">{t('nav_create_bounty')}</Link>
      <Link to="/leaderboard">{t('nav_leaderboard')}</Link>
      <Link to="/stats">{t('nav_stats')}</Link>
      <Link to="/my-bounties">{t('nav_my_bounties')}</Link>
      <Link to="/admin">{t('nav_admin')}</Link>
      <Link to="/create" role="button">{t('nav_create_bounty')}</Link>
      <WalletConnectButton address={address} onConnect={connect} />
      {error && (
        <p role="alert" className="wallet-connect-error">
          {error}
        </p>
      )}
    </nav>
  );
}

export default function App() {
  return (
    <WalletProvider>
      <BrowserRouter>
        <Nav />
        <NetworkMismatchBanner />
        <Routes>
          <Route path="/" element={<BountyList />} />
          <Route path="/bounties/:id" element={<BountyDetail />} />
          <Route path="/create" element={<CreateBounty />} />
          <Route path="/contributors/:address" element={<ContributorProfile />} />
          <Route path="/leaderboard" element={<Leaderboard />} />
          <Route path="/stats" element={<Stats />} />
          <Route path="/my-bounties" element={<MyBounties />} />
          <Route path="/admin" element={<Admin />} />
          <Route
            path="/"
            element={
              <BountyErrorBoundary>
                <BountyList />
              </BountyErrorBoundary>
            }
          />
          <Route
            path="/bounties/:id"
            element={
              <BountyErrorBoundary>
                <BountyDetail />
              </BountyErrorBoundary>
            }
          />
          <Route
            path="/create"
            element={
              <BountyErrorBoundary>
                <CreateBounty />
              </BountyErrorBoundary>
            }
          />
          <Route
            path="/contributors/:address"
            element={
              <BountyErrorBoundary>
                <ContributorProfile />
              </BountyErrorBoundary>
            }
          />
          <Route path="*" element={<NotFound />} />
        </Routes>
      </BrowserRouter>
    </WalletProvider>
  );
}
